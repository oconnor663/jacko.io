use anyhow::Context;
use pulldown_cmark::{
    BrokenLink, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::html::{styled_line_to_highlighted_html, IncludeBackground};
use syntect::parsing::SyntaxSet;
use url::Url;

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
<p><a href="/">↫ Home</a></p>
<h1>__TITLE__</h1>
<p class="subtitle">__SUBTITLE__</p>
<section>"#;

const FOOTER: &str = r#"

</section>
</article>
</body>
</html>
"#;

#[derive(Debug, serde::Deserialize)]
struct CargoTomlPackage {
    edition: String,
}

#[derive(Debug, serde::Deserialize)]
struct CargoToml {
    package: CargoTomlPackage,
}

fn playground_url(url: Url, markdown_filepath: &Path) -> anyhow::Result<String> {
    let rust_file = markdown_filepath
        .parent()
        .unwrap()
        .join(url.domain().expect("expected a domain (really a dirname)"))
        .join(url.path().trim_start_matches('/'));
    let code = fs::read_to_string(&rust_file)
        .context(format!("reading {}", rust_file.to_string_lossy()))?;
    // Use the "edition" field in Cargo.toml to set the edition query parameter.
    let cargo_toml_file = rust_file.parent().unwrap().join("Cargo.toml");
    let cargo_toml: CargoToml = toml::from_str(&fs::read_to_string(&cargo_toml_file).context(
        format!("reading file {}", cargo_toml_file.to_string_lossy()),
    )?)?;
    let edition = cargo_toml.package.edition;
    let mut ret = Url::parse("https://play.rust-lang.org")?;
    // Preserve supplied query parameters, for example mode=release.
    ret.set_query(url.query());
    ret.query_pairs_mut().append_pair("edition", &edition);
    ret.query_pairs_mut().append_pair("code", code.trim());
    Ok(ret.into())
}

fn link_url_to_escaped_href(
    url_str: impl Into<String>,
    markdown_filepath: &Path,
) -> anyhow::Result<String> {
    let url_string = url_str.into();
    let unescaped = match Url::parse(&url_string) {
        Ok(parsed) => {
            if parsed.scheme() == "playground" {
                playground_url(parsed, markdown_filepath)?
            } else {
                url_string
            }
        }
        Err(url::ParseError::RelativeUrlWithoutBase) => url_string,
        Err(e) => panic!("bad URL: {e}"),
    };
    Ok(html_escape::encode_double_quoted_attribute(&unescaped).to_string())
}

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
    markdown_filepath: PathBuf,
}

impl Output {
    fn new(markdown_filepath: impl Into<PathBuf>) -> Self {
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
            markdown_filepath: markdown_filepath.into(),
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

    fn finish_code_block(&mut self) -> anyhow::Result<()> {
        let Some(code_block) = self.current_code_block.take() else {
            panic!("not in a codeblock");
        };

        let code_lines = CodeLines::parse(&code_block.contents_text);

        self.document_html += "\n\n";
        if code_lines.is_wide() {
            self.document_html += r#"<pre class="fullwidth"><code>"#;
        } else {
            self.document_html += "<pre><code>";
        }

        if let Some(code_link) = &code_lines.link {
            self.document_html += &format!(
                r#"<div class="code_link"><a href="{}">{}</a></div>"#,
                link_url_to_escaped_href(&code_link.url, &self.markdown_filepath)?,
                html_escape::encode_text(&code_link.text),
            );
        }

        if !code_block.language.is_empty() {
            // The syntect syntax names have names like "Rust" and "C", not "rust" and "c". Make sure
            // (only) the first letter is capitalized.
            let mut capitalized_language = code_block.language;
            capitalized_language[0..1].make_ascii_uppercase();
            capitalized_language[1..].make_ascii_lowercase();

            // syntax highlighting
            // https://github.com/trishume/syntect/blob/c61ce60c72d67ad4e3dd06d60ff3b13ef4d2698c/examples/synhtml.rs
            let syntax_set = SyntaxSet::load_defaults_newlines();
            let syntax = syntax_set
                .find_syntax_by_name(&capitalized_language)
                .expect("unknown language name");
            let theme_set = ThemeSet::load_defaults();
            let mut line_highlighter =
                HighlightLines::new(syntax, &theme_set.themes["Solarized (light)"]);
            for (i, line_text) in code_lines.lines.iter().enumerate() {
                let ranges: Vec<(Style, &str)> = line_highlighter
                    .highlight_line(line_text, &syntax_set)
                    .unwrap();
                let line_html =
                    styled_line_to_highlighted_html(&ranges[..], IncludeBackground::No).unwrap();
                // Line numbers conventionally start with 1.
                let line_number = i + 1;
                if code_lines.highlighted_lines.is_faded(line_number) {
                    self.document_html += "<span class=\"faded_code\">";
                }
                self.document_html += &line_html;
                if code_lines.highlighted_lines.is_faded(line_number) {
                    self.document_html += "</span>";
                }
                self.document_html += "<br>";
            }
        } else {
            for (i, line_text) in code_lines.lines.iter().enumerate() {
                // Line numbers conventionally start with 1.
                // TODO: There's some unfortunate duplication across branches here.
                let line_number = i + 1;
                if code_lines.highlighted_lines.is_faded(line_number) {
                    self.document_html += "<span class=\"faded_code\">";
                }
                self.document_html += &html_escape::encode_text(line_text);
                if code_lines.highlighted_lines.is_faded(line_number) {
                    self.document_html += "</span>";
                }
                self.document_html += "<br>";
            }
        }

        self.document_html += "</code></pre>";
        Ok(())
    }
}

fn render_markdown(markdown_filepath: impl AsRef<Path>) -> anyhow::Result<String> {
    let markdown_input = fs::read_to_string(markdown_filepath.as_ref()).context(format!(
        "reading markdown file: {}",
        markdown_filepath.as_ref().to_string_lossy(),
    ))?;
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_with_broken_link_callback(
        &markdown_input,
        options,
        Some(|link: BrokenLink| {
            panic!("broken link: \"{}\"", link.reference);
        }),
    );

    let mut output = Output::new(markdown_filepath.as_ref());

    let mut nested_p_tag = false;
    let mut seen_link_ids = HashSet::new();
    for (event, _range) in parser.into_offset_iter() {
        match event {
            Event::Text(s) => output.push_text(&s),
            Event::Html(s) => output.push_html(&s),
            Event::InlineHtml(s) => output.push_html(&s),
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
                Tag::BlockQuote(_) => {
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
                Tag::Link { dest_url, id, .. } => {
                    if !id.is_empty() {
                        seen_link_ids.insert(id);
                    }
                    assert!(!dest_url.is_empty());
                    output.push_html(&format!(
                        r#"<a class="custom-link-color" href="{}">"#,
                        link_url_to_escaped_href(dest_url.as_ref(), markdown_filepath.as_ref())?,
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
                    output.finish_code_block()?;
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

    Ok(HEADER
        .replace("__TITLE__", &output.title_html)
        .replace("__SUBTITLE__", &output.subtitle_html)
        + &document_with_footnotes
        + FOOTER)
}

struct CodeLink {
    text: String,
    url: String,
}

struct HighlightedLines {
    line_numbers: Vec<usize>,
}

impl HighlightedLines {
    fn empty() -> Self {
        Self {
            line_numbers: Vec::new(),
        }
    }

    /// Parse an expression like "1,3-5" into [1,3,4,5].
    fn parse(ranges: &str) -> Self {
        let mut line_numbers = Vec::new();
        for element in ranges.split(',') {
            if element.contains('-') {
                // ranges like "3-5"
                let mut numbers = element.split('-');
                let start: usize = numbers
                    .next()
                    .expect("first number split")
                    .trim()
                    .parse()
                    .expect("first number parse");
                let end: usize = numbers
                    .next()
                    .expect("second number split")
                    .trim()
                    .parse()
                    .expect("second number parse");
                assert!(numbers.next().is_none());
                for i in start..=end {
                    // note: these ranges are inclusive
                    line_numbers.push(i);
                }
            } else if !element.is_empty() {
                // standalone numbers like "1"
                let number: usize = element.trim().parse().expect("number parse");
                line_numbers.push(number);
            }
        }
        Self { line_numbers }
    }

    /// Note that line numbers conventionally start from 1.
    fn is_faded(&self, line_number: usize) -> bool {
        !self.line_numbers.is_empty() && !self.line_numbers.contains(&line_number)
    }
}

struct CodeLines {
    link: Option<CodeLink>,
    lines: Vec<String>,
    highlighted_lines: HighlightedLines,
}

impl CodeLines {
    // Markdown doesn't make it easy to put anchor tags around an entire code block. Use a hacky
    // tags format like "LINK: " on the first lines of a codeblock as a workaround.
    fn parse(text: &str) -> CodeLines {
        let mut lines = text.lines().peekable();
        let link_tag = "LINK: ";
        let mut code_link = None;
        let highlight_tag = "HIGHLIGHT: ";
        let mut highlight_lines = HighlightedLines::empty();
        loop {
            let Some(next_line) = lines.peek() else {
                break;
            };
            if next_line.starts_with(link_tag) {
                // Consume the "LINK: " line.
                let link_line = lines.next().unwrap();
                let after_tag = &link_line[link_tag.len()..];
                let (text, url) = after_tag.rsplit_once(' ').expect("no link text?");
                assert_eq!(text, text.trim());
                code_link = Some(CodeLink {
                    text: text.into(),
                    url: url.into(),
                });
            } else if next_line.starts_with(highlight_tag) {
                // Consume the "HIGHLIGHT: " line.
                let highlight_line = lines.next().unwrap();
                let after_tag = &highlight_line[highlight_tag.len()..];
                highlight_lines = HighlightedLines::parse(after_tag);
            } else {
                // No more tags. The rest of the lines are text.
                break;
            }
        }
        CodeLines {
            link: code_link,
            lines: lines.map(String::from).collect(),
            highlighted_lines: highlight_lines,
        }
    }

    fn is_wide(&self) -> bool {
        self.lines.iter().any(|line| line.len() > 75)
    }
}

fn main() -> anyhow::Result<()> {
    let cargo_toml_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let posts_dir = cargo_toml_dir.join("../posts");
    let render_dir = cargo_toml_dir.join("../www");
    let mut post_paths = BTreeSet::new(); // sorted
    for entry in fs::read_dir(posts_dir)? {
        post_paths.insert(entry?.path());
    }
    for path in &post_paths {
        if path.extension() != Some("md".as_ref()) {
            continue;
        }
        let post_name = path.file_name().unwrap().to_string_lossy().to_string();
        println!("rendering {post_name}");
        let post_html = render_markdown(path)?;
        fs::write(
            render_dir.join(post_name.replace(".md", ".html")),
            &post_html,
        )?;
    }
    Ok(())
}
