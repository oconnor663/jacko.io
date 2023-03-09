use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::Path;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::html::{styled_line_to_highlighted_html, IncludeBackground};
use syntect::parsing::SyntaxSet;

const HEADER: &str = r#"<!DOCTYPE html>
<!-- rendered by https://github.com/oconnor663/jacko.io/blob/master/render_posts/src/main.rs -->
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<!-- tufte.css is adapted from https://github.com/edwardtufte/tufte-css, MIT licensed -->
<link rel="stylesheet" href="tufte.css">
<title>__TITLE__</title>
</head>
<body>
<article>
<p><a href="index.html">↫ Home</a></p>
<h1>__TITLE__</h1>
<p class="subtitle">__SUBTITLE__</p>
<section>"#;

const FOOTER: &str = r#"

</section>
</article>
</body>
</html>
"#;

struct Footnote {
    name: String,
    contents: String,
}

struct CodeBlock {
    language: String,
    contents: String,
}

struct Output {
    document: String,
    title: String,
    in_title: bool,
    subtitle: String,
    in_subtitle: bool,
    // Each footnote is parsed incrementally, just like the document is.
    current_footnote: Option<Footnote>,
    // Unfortunately code blocks are also parsed incrementally, which is kind of awkward.
    current_code_block: Option<CodeBlock>,
    // map of name to contents
    footnotes: HashMap<String, String>,
    // sorted map of offset to name
    footnote_references: BTreeMap<usize, String>,
}

impl Output {
    fn new() -> Self {
        Self {
            document: String::new(),
            title: String::new(),
            in_title: false,
            subtitle: String::new(),
            in_subtitle: false,
            current_footnote: None,
            current_code_block: None,
            footnotes: HashMap::new(),
            footnote_references: BTreeMap::new(),
        }
    }

    fn push_str(&mut self, text: &str) {
        if self.in_title {
            assert!(!self.in_subtitle);
            self.title += text;
        } else if self.in_subtitle {
            assert!(!self.in_title);
            self.subtitle += text;
        } else if let Some(footnote) = &mut self.current_footnote {
            footnote.contents += text;
        } else if let Some(code_block) = &mut self.current_code_block {
            code_block.contents += text;
        } else {
            self.document += text;
        }
    }

    fn start_footnote(&mut self, name: String) {
        assert!(!self.in_title);
        assert!(!self.in_subtitle);
        assert!(self.current_footnote.is_none(), "already in a footnote");
        assert!(self.current_code_block.is_none(), "already in a codeblock");
        assert!(
            !self.footnotes.contains_key(&name),
            "footnote {name} already exists",
        );
        self.current_footnote = Some(Footnote {
            name,
            contents: String::new(),
        });
    }

    fn finish_footnote(&mut self, name: String) {
        let Some(footnote) = self.current_footnote.take() else {
            panic!("not in a footnote");
        };
        assert_eq!(name, footnote.name, "name mismatch");
        let trimmed_note = footnote
            .contents
            .trim()
            .trim_start_matches("<p>")
            .trim_end_matches("</p>");
        self.footnotes
            .insert(footnote.name, trimmed_note.to_string());
    }

    fn add_footnote_reference(&mut self, name: String) {
        assert!(self.current_footnote.is_none(), "no footnotes in footnotes");
        let offset = self.document.len();
        self.footnote_references.insert(offset, name);
    }

    fn validate_footnotes(&self) {
        for name in self.footnote_references.values() {
            assert!(
                self.footnotes.contains_key(name),
                "reference to unknown footnote {name}",
            );
        }
        let referenced_names: HashSet<String> = self
            .footnote_references
            .iter()
            .map(|(_offset, name)| name.clone())
            .collect();
        for name in self.footnotes.keys() {
            assert!(
                referenced_names.contains(name),
                "footnote {name} has no references",
            );
        }
    }

    fn start_code_block(&mut self, language: String) {
        assert!(!self.in_title);
        assert!(!self.in_subtitle);
        assert!(self.current_code_block.is_none(), "already in a codeblock");
        assert!(self.current_footnote.is_none(), "already in a footnote");
        self.current_code_block = Some(CodeBlock {
            language,
            contents: String::new(),
        });
    }

    fn finish_code_block(&mut self) {
        let Some(code_block) = self.current_code_block.take() else {
            panic!("not in a codeblock");
        };

        // The syntect syntax names have names like "Rust" and "C", not "rust" and "c". Make sure
        // (only) the first letter is capitalized.
        let mut first = true;
        let capitalized_language: String = code_block
            .language
            .chars()
            .map(|c| {
                if first {
                    first = false;
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect();

        self.document += "\n\n<pre><code>";

        // syntax highlighting
        // https://github.com/trishume/syntect/blob/c61ce60c72d67ad4e3dd06d60ff3b13ef4d2698c/examples/synhtml.rs
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntax = syntax_set
            .find_syntax_by_name(&capitalized_language)
            .unwrap();
        let theme_set = ThemeSet::load_defaults();
        let mut line_highlighter =
            HighlightLines::new(syntax, &theme_set.themes["Solarized (light)"]);
        for line in code_block.contents.lines() {
            let ranges: Vec<(Style, &str)> =
                line_highlighter.highlight_line(line, &syntax_set).unwrap();
            let html = styled_line_to_highlighted_html(&ranges[..], IncludeBackground::No).unwrap();
            self.document += &html;
            self.document += "<br>";
        }

        self.document += "</code></pre>";
    }
}

fn render_markdown(markdown_input: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(markdown_input, options);

    let mut output = Output::new();

    let mut nested_p_tag = false;
    for (event, _range) in parser.into_offset_iter() {
        match event {
            Event::Text(s) => output.push_str(&s),
            Event::Html(s) => output.push_str(&s),
            Event::SoftBreak => output.push_str("\n"),
            Event::Code(s) => output.push_str(&format!("<code>{s}</code>")),
            Event::Rule => output.push_str(&format!("\n\n<hr>")),
            Event::FootnoteReference(s) => {
                output.add_footnote_reference(s.to_string());
            }
            Event::Start(tag) => match tag {
                Tag::BlockQuote => {
                    output.push_str("\n\n<blockquote>");
                    nested_p_tag = true;
                }
                Tag::Paragraph => {
                    if nested_p_tag {
                        nested_p_tag = false;
                    } else {
                        output.push_str("\n\n");
                    }
                    output.push_str("<p>");
                }
                Tag::Heading(level, _fragment, _class) => {
                    if level == HeadingLevel::H1 {
                        output.in_title = true;
                    } else if level == HeadingLevel::H6 {
                        output.in_subtitle = true;
                    } else {
                        output.push_str(&format!("\n</section>\n\n<section>\n<{level}>"));
                    }
                }
                Tag::Strong => output.push_str("<strong>"),
                Tag::Emphasis => output.push_str("<em>"),
                Tag::Link(kind, dest, _title) => {
                    assert_eq!(kind, LinkType::Inline);
                    output.push_str(&format!(r#"<a class="custom-link-color" href="{dest}">"#));
                }
                Tag::CodeBlock(kind) => {
                    let CodeBlockKind::Fenced(language) = kind else {
                        panic!("unsupported code block: {:?}", kind);
                    };
                    output.start_code_block(language.to_string());
                }
                Tag::List(_) => output.push_str("\n\n<ul>"),
                Tag::Item => output.push_str("\n<li>"),
                Tag::FootnoteDefinition(s) => {
                    output.start_footnote(s.to_string());
                }
                other => unimplemented!("{:?}", other),
            },
            Event::End(tag) => match tag {
                Tag::BlockQuote => output.push_str("</blockquote>"),
                Tag::Paragraph => output.push_str("</p>"),
                Tag::Heading(level, _fragment, _class) => {
                    if level == HeadingLevel::H1 {
                        output.in_title = false;
                    } else if level == HeadingLevel::H6 {
                        output.in_subtitle = false;
                    } else {
                        output.push_str(&format!("</{}>", level));
                    }
                }
                Tag::Strong => output.push_str("</strong>"),
                Tag::Emphasis => output.push_str("</em>"),
                Tag::Link(_kind, _dest, _title) => output.push_str("</a>"),
                Tag::CodeBlock(_kind) => {
                    output.finish_code_block();
                }
                Tag::List(_) => output.push_str("\n</ul>"),
                Tag::Item => output.push_str("</li>"),
                Tag::FootnoteDefinition(s) => {
                    output.finish_footnote(s.to_string());
                }
                other => unimplemented!("{:?}", other),
            },
            other => unimplemented!("{:?}", other),
        }
    }

    output.validate_footnotes();

    let mut document_with_footnotes = String::new();
    let mut current_offset = 0;
    let mut already_seen: HashSet<String> = HashSet::new();
    for (offset, name) in output.footnote_references {
        document_with_footnotes += &output.document[current_offset..offset];
        current_offset = offset;
        document_with_footnotes += &format!(
            r#"<label for="sidenote-{name}" class="margin-toggle sidenote-number"></label><input type="checkbox" id="sidenote-{name}" class="margin-toggle">"#,
        );
        if !already_seen.contains(&name) {
            document_with_footnotes += r#"<span class="sidenote">"#;
            document_with_footnotes += &output.footnotes[&name];
            document_with_footnotes += r#"</span>"#;
            already_seen.insert(name);
        }
    }
    document_with_footnotes += &output.document[current_offset..];

    HEADER
        .replace("__TITLE__", &output.title)
        .replace("__SUBTITLE__", &output.subtitle)
        + &document_with_footnotes
        + FOOTER
}

fn main() -> anyhow::Result<()> {
    let cargo_toml_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let posts_dir = cargo_toml_dir.join("../posts");
    let render_dir = cargo_toml_dir.join("../www");
    for post_entry in fs::read_dir(posts_dir)? {
        let post_entry = post_entry?;
        let post_name = post_entry.file_name().to_string_lossy().to_string();
        println!("rendering {post_name}");
        let post_markdown = fs::read_to_string(post_entry.path())?;
        let post_html = render_markdown(&post_markdown);
        fs::write(
            render_dir.join(post_name.replace(".md", ".html")),
            &post_html,
        )?;
    }
    Ok(())
}
