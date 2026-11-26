//! 文件 I/O 边界：文本解码、换行规范化、指纹与原子保存。
use super::document::{Encoding, Newline};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// 打开或保存成功时记下的磁盘快照，用来判断文件有没有被外部改过。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FileFingerprint {
    pub(super) length: u64,
    /// 取不到修改时间时为 `None`，比较时会把它当成差异的一部分。
    pub(super) modified: Option<SystemTime>,
    /// 文件全部字节的哈希，避免只靠长度和修改时间误判。
    pub(super) content_hash: u64,
}

/// 从磁盘读取并规范化后的文本及其原始格式信息。
#[derive(Debug)]
pub(super) struct LoadedText {
    pub(super) path: PathBuf,
    pub(super) text: String,
    pub(super) encoding: Encoding,
    pub(super) newline: Newline,
    pub(super) fingerprint: FileFingerprint,
}

/// 读取路径上的字节并计算指纹。两次读取之间文件被替换时，调用方应视为已变化。
pub(super) fn fingerprint(path: &Path) -> io::Result<FileFingerprint> {
    read_snapshot(path).map(|(_, fingerprint)| fingerprint)
}

fn read_snapshot(path: &Path) -> io::Result<(Vec<u8>, FileFingerprint)> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let fingerprint = fingerprint_bytes(&bytes, file.metadata()?.modified().ok());
    Ok((bytes, fingerprint))
}

fn fingerprint_bytes(bytes: &[u8], modified: Option<SystemTime>) -> FileFingerprint {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    FileFingerprint {
        length: bytes.len() as u64,
        modified,
        content_hash: hasher.finish(),
    }
}

/// 按指定或自动识别的编码读取文本，并将 CRLF 规范化为编辑器内部的 LF。
pub(super) fn load_with_encoding(
    path: &Path,
    requested: Option<Encoding>,
) -> io::Result<LoadedText> {
    let (bytes, fingerprint) = read_snapshot(path)?;
    let (encoding, text) = Encoding::decode(&bytes, requested)?;
    // 去掉成对 CRLF 后仍有 CR，或同一文件里同时出现 CRLF 和单独 LF，都拒绝读取。
    let unpaired = text.replace("\r\n", "");
    if unpaired.contains('\r') || (text.contains("\r\n") && unpaired.contains('\n')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "暂不支持混合换行或单独 CR；原文件未修改",
        ));
    }
    let newline = if text.contains("\r\n") {
        Newline::Crlf
    } else {
        Newline::Lf
    };
    Ok(LoadedText {
        path: path.to_path_buf(),
        text: text.replace("\r\n", "\n"),
        encoding,
        newline,
        fingerprint,
    })
}

#[cfg(test)]
pub(super) fn load(path: &Path) -> io::Result<LoadedText> {
    load_with_encoding(path, None)
}

/// 磁盘内容仍等于 `expected` 时才写入。
///
/// `expected` 为 `None` 表示新文件，不做外部修改检查。
/// 指纹不一致，或文件已经被删掉，都拒绝覆盖并保留磁盘现状。
pub(super) fn save_if_unchanged(
    path: &Path,
    bytes: &[u8],
    expected: Option<&FileFingerprint>,
) -> io::Result<(PathBuf, FileFingerprint)> {
    if let Some(expected) = expected {
        let unchanged = fingerprint(path)
            .map(|actual| actual == *expected)
            .unwrap_or(false);
        if !unchanged {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "文件已被外部修改或删除，未覆盖磁盘内容；请使用另存为保存当前修改",
            ));
        }
    }
    save(path, bytes)
}

/// 将字节通过同目录临时文件原子替换写入目标路径。
fn save(path: &Path, bytes: &[u8]) -> io::Result<(PathBuf, FileFingerprint)> {
    use std::io::Write;
    // 已有符号链接时写到解析后的目标，避免把链接本身替换成普通文件。
    let target = match std::fs::symlink_metadata(path) {
        Ok(_) => std::fs::canonicalize(path)?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => path.to_path_buf(),
        Err(e) => return Err(e),
    };
    let metadata = match std::fs::metadata(&target) {
        Ok(meta) => Some(meta),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    if metadata
        .as_ref()
        .is_some_and(|meta| meta.permissions().readonly())
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "文件为只读",
        ));
    }
    let mut temp = tempfile::NamedTempFile::new_in(target.parent().unwrap_or(Path::new(".")))?;
    temp.write_all(bytes)?;
    // 原子替换会换掉 inode，先把原权限拷到临时文件上。
    if let Some(meta) = metadata {
        temp.as_file().set_permissions(meta.permissions())?;
    }
    temp.as_file().sync_all()?;
    let modified = temp.as_file().metadata()?.modified().ok();
    temp.persist(&target).map_err(|e| e.error)?;
    Ok((target, fingerprint_bytes(bytes, modified)))
}

/// 按文档编码和换行设置生成最终写入磁盘的字节序列。
pub(super) fn bytes_for_save(
    text: &str,
    encoding: Encoding,
    newline: Newline,
) -> io::Result<Vec<u8>> {
    match newline {
        Newline::Lf => encoding.encode(text),
        Newline::Crlf => encoding.encode(&text.replace('\n', "\r\n")),
    }
}
