//! 文件 I/O 边界：只处理 UTF-8/BOM 和 LF/CRLF，保留明确的失败信息。
use super::document::{Encoding, Newline};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
/// 从磁盘读取并规范化后的文本及其原始格式信息。
pub(super) struct LoadedText {
    pub(super) path: PathBuf,
    pub(super) text: String,
    pub(super) encoding: Encoding,
    pub(super) newline: Newline,
}

/// 读取文本文件，校验 UTF-8，并将 CRLF 规范化为编辑器内部的 LF。
pub(super) fn load(path: &Path) -> io::Result<LoadedText> {
    let bytes = std::fs::read(path)?;
    let (encoding, body) = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        (Encoding::Utf8Bom, &bytes[3..])
    } else {
        (Encoding::Utf8, bytes.as_slice())
    };
    let text = String::from_utf8(body.to_vec()).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("文件不是有效 UTF-8：{error}"),
        )
    })?;
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
    })
}

/// 同目录原子替换。解析符号链接后写入目标，拒绝只读文件。
/// 将字节通过同目录临时文件原子替换写入目标路径。
pub(super) fn save(path: &Path, bytes: &[u8]) -> io::Result<PathBuf> {
    use std::io::Write;
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
    if let Some(meta) = metadata {
        temp.as_file().set_permissions(meta.permissions())?;
    }
    temp.as_file().sync_all()?;
    temp.persist(&target).map_err(|e| e.error)?;
    Ok(target)
}

/// 按文档编码和换行设置生成最终写入磁盘的字节序列。
pub(super) fn bytes_for_save(text: &str, encoding: Encoding, newline: Newline) -> Vec<u8> {
    let normalized = match newline {
        Newline::Lf => text.to_owned(),
        Newline::Crlf => text.replace('\n', "\r\n"),
    };
    let mut bytes = Vec::with_capacity(normalized.len() + 3);
    if encoding == Encoding::Utf8Bom {
        bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    }
    bytes.extend_from_slice(normalized.as_bytes());
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_bom_and_newline_on_save() {
        assert_eq!(
            bytes_for_save("a\nb", Encoding::Utf8Bom, Newline::Crlf),
            b"\xEF\xBB\xBFa\r\nb"
        );
    }
    #[test]
    fn rejects_invalid_utf8() {
        let file = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(file.path(), [0xff, 0xfe]).unwrap();
        let result = load(file.path());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    }
}
