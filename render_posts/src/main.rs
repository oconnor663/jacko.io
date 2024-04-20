use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
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
    contents_html: String,
}

struct CodeBlock {
    language: String,
    contents_text: String,
}

struct Output {
    document_html: String,
    title_html: String,
    in_title: bool,
    subtitle_html: String,
    in_subtitle: bool,
    // Each footnote is parsed incrementally, just like the document is.
    current_footnote: Option<Footnote>,
    // Unfortunately code blocks are also parsed incrementally, which is kind of awkward.
    current_code_block: Option<CodeBlock>,
    // map of name to contents
    footnotes: HashMap<String, Footnote>,
    // sorted map of offset to name
    footnote_references: BTreeMap<usize, Vec<String>>,
}

impl Output {
    fn new() -> Self {
        Self {
            document_html: String::new(),
            title_html: String::new(),
            in_title: false,
            subtitle_html: String::new(),
            in_subtitle: false,
            current_footnote: None,
            current_code_block: None,
            footnotes: HashMap::new(),
            footnote_references: BTreeMap::new(),
        }
    }

    fn push_text(&mut self, text: &str) {
        if let Some(code_block) = &mut self.current_code_block {
            code_block.contents_text += text;
        } else {
            self.push_html(&html_escape::encode_text(text));
        }
    }

    fn push_html(&mut self, html: &str) {
        assert!(
            self.current_code_block.is_none(),
            "code blocks only take text"
        );
        if self.in_title {
            assert!(!self.in_subtitle);
            self.title_html += html;
        } else if self.in_subtitle {
            assert!(!self.in_title);
            self.subtitle_html += html;
        } else if let Some(footnote) = &mut self.current_footnote {
            footnote.contents_html += html;
        } else {
            self.document_html += html;
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
            contents_html: String::new(),
        });
    }

    fn finish_footnote(&mut self) {
        let Some(footnote) = self.current_footnote.take() else {
            panic!("not in a footnote");
        };
        let trimmed_html = footnote
            .contents_html
            .trim()
            .trim_start_matches("<p>")
            .trim_end_matches("</p>");
        let trimmed_footnote = Footnote {
            name: footnote.name.clone(),
            contents_html: trimmed_html.to_string(),
        };
        self.footnotes
            .insert(footnote.name.clone(), trimmed_footnote);
    }

    fn add_footnote_reference(&mut self, name: String) {
        assert!(self.current_footnote.is_none(), "no footnotes in footnotes");
        let offset = self.document_html.len();
        self.footnote_references
            .entry(offset)
            .or_insert(Vec::new())
            .push(name);
    }

    fn validate_footnotes(&self) {
        let mut referenced_names = HashSet::new();
        for names in self.footnote_references.values() {
            for name in names {
                assert!(
                    self.footnotes.contains_key(name),
                    "reference to unknown footnote {name}",
                );
                referenced_names.insert(name.clone());
            }
        }
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
            contents_text: String::new(),
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

        self.document_html += "\n\n<pre><code>";

        if !capitalized_language.is_empty() {
            // syntax highlighting
            // https://github.com/trishume/syntect/blob/c61ce60c72d67ad4e3dd06d60ff3b13ef4d2698c/examples/synhtml.rs
            let syntax_set = SyntaxSet::load_defaults_newlines();
            let syntax = syntax_set
                .find_syntax_by_name(&capitalized_language)
                .expect("unknown language name");
            let theme_set = ThemeSet::load_defaults();
            let mut line_highlighter =
                HighlightLines::new(syntax, &theme_set.themes["Solarized (light)"]);
            for line_text in code_block.contents_text.lines() {
                let ranges: Vec<(Style, &str)> = line_highlighter
                    .highlight_line(line_text, &syntax_set)
                    .unwrap();
                let line_html =
                    styled_line_to_highlighted_html(&ranges[..], IncludeBackground::No).unwrap();
                self.document_html += &line_html;
                self.document_html += "<br>";
            }
        } else {
            for line_text in code_block.contents_text.lines() {
                self.document_html += &html_escape::encode_text(line_text);
                self.document_html += "<br>";
            }
        }

        self.document_html += "</code></pre>";
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
            Event::Text(s) => output.push_text(&s),
            Event::Html(s) => output.push_html(&s),
            Event::SoftBreak => output.push_html("\n"),
            Event::Code(s) => {
                for (i, word) in s.split_whitespace().enumerate() {
                    if i > 0 {
                        // Full size spaces in inline code strings are uncomfortably wide. Hack in
                        // shorter spaces.
                        output.push_html("&nbsp;");
                    }
                    output.push_html(&format!("<code>{}</code>", html_escape::encode_text(&word)));
                }
            }
            Event::Rule => output.push_html(&format!("\n\n<hr>")),
            Event::FootnoteReference(s) => {
                output.add_footnote_reference(s.to_string());
            }
            Event::Start(tag) => match tag {
                Tag::BlockQuote => {
                    output.push_html("\n\n<blockquote>");
                    nested_p_tag = true;
                }
                Tag::Paragraph => {
                    if nested_p_tag {
                        nested_p_tag = false;
                    } else {
                        output.push_html("\n\n");
                    }
                    output.push_html("<p>");
                }
                Tag::Heading { level, .. } => {
                    if level == HeadingLevel::H1 {
                        output.in_title = true;
                    } else if level == HeadingLevel::H6 {
                        output.in_subtitle = true;
                    } else {
                        output.push_html(&format!("\n</section>\n\n<section>\n<{level}>"));
                    }
                }
                Tag::Strong => output.push_html("<strong>"),
                Tag::Emphasis => output.push_html("<em>"),
                Tag::Link { dest_url, .. } => {
                    assert!(!dest_url.is_empty());
                    output.push_html(&format!(
                        r#"<a class="custom-link-color" href="{dest_url}">"#
                    ));
                }
                Tag::CodeBlock(kind) => {
                    let CodeBlockKind::Fenced(language) = kind else {
                        panic!("unsupported code block: {:?}", kind);
                    };
                    output.start_code_block(language.to_string());
                }
                Tag::List(_) => output.push_html("\n\n<ul>"),
                Tag::Item => output.push_html("\n<li>"),
                Tag::FootnoteDefinition(s) => {
                    output.start_footnote(s.to_string());
                }
                other => unimplemented!("{:?}", other),
            },
            Event::End(tag) => match tag {
                TagEnd::BlockQuote => output.push_html("</blockquote>"),
                TagEnd::Paragraph => output.push_html("</p>"),
                TagEnd::Heading(level) => {
                    if level == HeadingLevel::H1 {
                        output.in_title = false;
                    } else if level == HeadingLevel::H6 {
                        output.in_subtitle = false;
                    } else {
                        output.push_html(&format!("</{}>", level));
                    }
                }
                TagEnd::Strong => output.push_html("</strong>"),
                TagEnd::Emphasis => output.push_html("</em>"),
                TagEnd::Link => output.push_html("</a>"),
                TagEnd::CodeBlock => {
                    output.finish_code_block();
                }
                TagEnd::List(_) => output.push_html("\n</ul>"),
                TagEnd::Item => output.push_html("</li>"),
                TagEnd::FootnoteDefinition => {
                    output.finish_footnote();
                }
                other => unimplemented!("{:?}", other),
            },
            other => unimplemented!("{:?}", other),
        }
    }

    output.validate_footnotes();

    let mut document_with_footnotes = String::new();
    let mut current_offset = 0;
    let mut already_seen: HashSet<&str> = HashSet::new();
    for (&offset, names) in &output.footnote_references {
        for name in names {
            document_with_footnotes += &output.document_html[current_offset..offset];
            document_with_footnotes += r#"<span style="white-space: nowrap">"#;
            if current_offset == offset {
                // If there's more than one footnote at the same point in the text, put a space in
                // between them.
                document_with_footnotes += " ";
            } else {
                // We don't want a space before the first footnote, but we still need something
                // here for nowrap to work.
                document_with_footnotes += "&ZeroWidthSpace;";
            }
            current_offset = offset;
            document_with_footnotes += &format!(
                r#"<label for="sidenote-{name}" class="margin-toggle sidenote-number"></label><input type="checkbox" id="sidenote-{name}" class="margin-toggle">"#,
            );
            if !already_seen.contains(name.as_str()) {
                document_with_footnotes += r#"<span class="sidenote" style="white-space: normal">"#;
                document_with_footnotes += &output.footnotes[name].contents_html;
                document_with_footnotes += r#"</span>"#;
                already_seen.insert(name);
            }
            document_with_footnotes += "</span>";
        }
    }
    document_with_footnotes += &output.document_html[current_offset..];

    HEADER
        .replace("__TITLE__", &output.title_html)
        .replace("__SUBTITLE__", &output.subtitle_html)
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
