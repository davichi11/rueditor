//! 应用拥有的语法注册与查询补充；不修改依赖 crate 的内置资源。
use gpui_kit::component::highlighter::{GrammarConfig, LanguageRegistry};
use std::sync::Once;

pub(super) fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let registry = LanguageRegistry::singleton();
        registry.register(
            "xml",
            &GrammarConfig::new(
                "xml",
                tree_sitter_xml::LANGUAGE_XML.into(),
                vec![],
                &tree_sitter_xml::XML_HIGHLIGHT_QUERY
                    .replace("@markup.link", "@link_uri")
                    .replace("@markup.heading", "@punctuation.special")
                    .replace("@markup.raw", "@string")
                    .replace("@markup", "@text.literal"),
                "",
                "",
            ),
        );
        registry.register(
            "properties",
            &GrammarConfig::new(
                "properties",
                tree_sitter_properties::LANGUAGE.into(),
                vec![],
                tree_sitter_properties::HIGHLIGHTS_QUERY,
                "",
                "",
            ),
        );
        registry.register(
            "ini",
            &GrammarConfig::new(
                "ini",
                tree_sitter_ini::LANGUAGE.into(),
                vec![],
                include_str!("languages/ini.scm"),
                "",
                "",
            ),
        );
        registry.register(
            "dotenv",
            &GrammarConfig::new(
                "dotenv",
                tree_sitter_bash::LANGUAGE.into(),
                vec![],
                include_str!("languages/dotenv.scm"),
                "",
                "",
            ),
        );
        // Several built-ins expose only a parser or an extension query.
        // Install full queries before constructing editor highlighters.
        for (name, highlights) in [
            ("csharp", tree_sitter_c_sharp::HIGHLIGHTS_QUERY),
            ("swift", tree_sitter_swift::HIGHLIGHTS_QUERY),
            ("cmake", include_str!("languages/cmake.scm")),
            ("proto", include_str!("languages/proto.scm")),
            ("graphql", include_str!("languages/graphql.scm")),
        ] {
            let mut config = registry.language(name).expect("enabled grammar");
            config.highlights = highlights.into();
            registry.register(name, &config);
        }
        let mut cpp = registry.language("cpp").expect("enabled C++ grammar");
        cpp.highlights = format!("{}\n{}", cpp.highlights, registry.language("c").expect("enabled C grammar").highlights).into();
        registry.register("cpp", &cpp);
        for name in ["ruby", "lua", "scala"] {
            let mut config = registry.language(name).expect("enabled grammar");
            // Internal local.* captures otherwise mask visual token roles.
            config.locals = "".into();
            registry.register(name, &config);
        }
        for name in ["c", "cpp", "java", "go", "csharp", "kotlin", "swift", "php", "ruby", "lua", "scala", "elixir", "zig", "sql", "bash", "make", "cmake", "proto", "graphql", "diff"] {
            let mut config = registry.language(name).expect("enabled grammar");
            // Spell-check metadata isn't a visual capture. In this renderer
            // its earlier same-range capture otherwise hides the real color.
            if name == "elixir" {
                config.highlights = format!("{}\n{}", include_str!("languages/elixir.scm"), config.highlights).into();
            }
            let mut query = config.highlights.replace(" @spell", "").replace(" @nospell", "");
            if name == "ruby" {
                // Without semantic local tracking, this catch-all would color
                // every parameter/local as a method. Explicit call/declaration
                // patterns below still identify real methods.
                query = query.replace("((identifier) @function.method\n (#is-not? local))", "");
            }
            let query = adapt_capture_names(&query);
            // Specific methods/types must precede generic identifier captures.
            let mut generic = Vec::new();
            let mut specific = Vec::new();
            for line in query.lines() {
                if matches!(line.trim(), "(identifier) @variable" | "(simple_identifier) @variable") {
                    generic.push(line);
                } else { specific.push(line); }
            }
            specific.extend(generic);
            config.highlights = specific.join("\n").into();
            registry.register(name, &config);
        }
        let typescript = registry.language("typescript").expect("enabled TypeScript grammar");
        for language in ["javascript", "typescript", "tsx"] {
            let mut config = registry
                .language(language)
                .expect("enabled built-in language");
            // Upstream TSX only supplies TypeScript-specific rules, omitting
            // ECMAScript entirely. Reuse the full TS rules with the TSX parser.
            if language == "tsx" {
                config.highlights = typescript.highlights.clone();
                config.injections = typescript.injections.clone();
                config.injection_languages = typescript.injection_languages.clone();
            }
            // Whole-template captures hide the semantics inside ${...}. Style
            // literal fragments instead and leave expressions to normal rules.
            let generic = "\n(identifier) @variable\n(property_identifier) @property\n(shorthand_property_identifier) @property\n(shorthand_property_identifier_pattern) @property\n(private_property_identifier) @property\n";
            let mut base = config.highlights.to_string()
                .replace("(template_string)", "(string_fragment)")
                .replace(") @embedded", ")")
                .replace("(regex) @string.special", "(regex) @string.regex");
            base = base.lines()
                .filter(|line| !generic.lines().any(|rule| !rule.is_empty() && line.trim() == rule))
                .collect::<Vec<_>>().join("\n");
            base.push('\n');
            // This renderer keeps the first capture at an identical range.
            // Put specific roles before generic identifiers/properties. Its
            // combined query also exposes local.* as visual captures. Omit
            // those internal scopes/references here so they cannot mask roles.
            config.locals = "".into();
            let mut highlights = String::from(include_str!("languages/ecmascript.scm"));
            highlights.push_str(if language == "javascript" {
                "\n(formal_parameters (identifier) @variable.parameter)\n(arrow_function parameter: (identifier) @variable.parameter)\n"
            } else {
                "\n(required_parameter pattern: (identifier) @variable.parameter)\n(optional_parameter pattern: (identifier) @variable.parameter)\n"
            });
            if language != "typescript" {
                highlights.push_str(include_str!("languages/jsx.scm"));
            }
            highlights.push_str(&base);
            highlights.push_str("\n(primary_expression/identifier) @variable\n(property_identifier) @property\n(shorthand_property_identifier) @property\n(shorthand_property_identifier_pattern) @property\n(private_property_identifier) @property\n");
            config.highlights = highlights.into();
            registry.register(language, &config);
            if language == "javascript" {
                config.name = "jsx".into();
                registry.register("jsx", &config);
            }
        }
    });
}

/// Map legacy grammar scopes onto the theme's supported semantic families.
/// Replace complete capture tokens (including predicate references), never
/// substrings such as `@parameter` inside an unrelated longer capture.
fn adapt_capture_names(query: &str) -> String {
    static CAPTURE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    CAPTURE
        .get_or_init(|| regex::Regex::new(r"@([A-Za-z_][A-Za-z0-9_.]*)").expect("capture pattern"))
        .replace_all(query, |captures: &regex::Captures<'_>| {
            let name = &captures[1];
            let mapped = match name {
                "namespace" | "module" => "type",
                "parameter" => "variable.parameter",
                "field" => "property",
                "method" | "method.call" => "function.method",
                "conditional" | "repeat" | "exception" | "include" | "storageclass" => "keyword",
                "float" => "number",
                "escape" | "character.special" => "string.escape",
                "string.regexp" | "string.special.regex" => "string.regex",
                _ => name,
            };
            format!("@{mapped}")
        })
        .into_owned()
}
