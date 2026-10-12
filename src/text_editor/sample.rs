//! 开发期验证样例；不作为真实用户文件或已保存文档。

#[allow(dead_code)]
pub(super) const SAMPLE_RUST: &str = r#"// Rueditor preview
fn main() {
    let message = "Hello from GPUI";
    println!("{message}");
    if message.len() > 0 {
        println!("syntax highlighting and folding");
    }
}
"#;

#[allow(dead_code)]
pub(super) const SAMPLES: [(&str, &str, &str); 3] = [
    ("rust", "sample.rs", SAMPLE_RUST),
    (
        "python",
        "sample.py",
        "def greet(name):\n    if name:\n        print(f'你好，{name}')\n    return name\n\ngreet('GPUI')\n",
    ),
    (
        "json",
        "sample.json",
        "{\n  \"editor\": {\n    \"name\": \"Rueditor\",\n    \"languages\": [\n      \"Rust\",\n      \"Python\"\n    ]\n  }\n}\n",
    ),
];
