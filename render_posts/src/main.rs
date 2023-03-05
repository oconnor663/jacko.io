use pulldown_cmark::{Event, HeadingLevel, LinkType, Options, Parser, Tag};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::prelude::*;

const HEADER: &str = r#"<html>
<head>
<link rel="stylesheet" href="file:///home/jacko/tufte-css/tufte.css"/>
<style>
a.custom-link-color {
    color: #144bb8;
}
</style>
</head>
<body>
<article>"#;

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

struct Output {
    document: String,
    // Each footnote is parsed incrementally, just like the document is.
    current_footnote: Option<Footnote>,
    // map of name to contents
    footnotes: HashMap<String, String>,
    // sorted map of offset to name
    footnote_references: BTreeMap<usize, String>,
}

impl Output {
    fn new() -> Self {
        Self {
            document: String::new(),
            current_footnote: None,
            footnotes: HashMap::new(),
            footnote_references: BTreeMap::new(),
        }
    }

    fn push_str(&mut self, text: &str) {
        if let Some(footnote) = &mut self.current_footnote {
            footnote.contents += text;
        } else {
            self.document += text;
        }
    }

    fn start_footnote(&mut self, name: String) {
        assert!(self.current_footnote.is_none(), "already in a footnote");
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
}

fn main() -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(&input, options);

    let mut output = Output::new();
    output.push_str(HEADER);

    let mut nested_p_tag = false;
    for (event, _range) in parser.into_offset_iter() {
        match event {
            Event::Text(s) => output.push_str(&s),
            Event::Html(s) => output.push_str(&s),
            Event::SoftBreak => output.push_str("\n"),
            Event::Code(s) => output.push_str(&format!("<code>{s}</code>")),
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
                    if level == HeadingLevel::H2 {
                        output.push_str(&format!("\n</section>"));
                    }
                    output.push_str("\n\n");
                    if level == HeadingLevel::H2 {
                        output.push_str("<section>\n");
                    }
                    output.push_str(&format!("<{level}>"));
                }
                Tag::Strong => output.push_str("<strong>"),
                Tag::Emphasis => output.push_str("<em>"),
                Tag::Link(kind, dest, _title) => {
                    assert_eq!(kind, LinkType::Inline);
                    output.push_str(&format!(
                        r#"<a class="custom-link-color no-tufte-underline" href="{dest}">"#
                    ));
                }
                Tag::CodeBlock(_kind) => output.push_str("\n\n<pre><code>"),
                Tag::List(_) => output.push_str("<ul>\n"),
                Tag::Item => output.push_str("<li>\n"),
                Tag::FootnoteDefinition(s) => {
                    output.start_footnote(s.to_string());
                }
                other => unimplemented!("{:?}", other),
            },
            Event::End(tag) => match tag {
                Tag::BlockQuote => output.push_str("</blockquote>"),
                Tag::Paragraph => output.push_str("</p>"),
                Tag::Heading(level, _fragment, _class) => {
                    output.push_str(&format!("</{}>", level));
                    if level == HeadingLevel::H1 {
                        output.push_str(&format!("\n\n<section>"));
                    }
                }
                Tag::Strong => output.push_str("</strong>"),
                Tag::Emphasis => output.push_str("</em>"),
                Tag::Link(_kind, _dest, _title) => output.push_str("</a>"),
                Tag::CodeBlock(_kind) => output.push_str("</code></pre>"),
                Tag::List(_) => output.push_str("</ul>"),
                Tag::Item => output.push_str("</li>"),
                Tag::FootnoteDefinition(s) => {
                    output.finish_footnote(s.to_string());
                }
                other => unimplemented!("{:?}", other),
            },
            other => unimplemented!("{:?}", other),
        }
    }

    output.document += FOOTER;

    output.validate_footnotes();

    let mut document_with_footnotes = String::new();
    let mut current_offset = 0;
    let mut already_seen: HashSet<String> = HashSet::new();
    for (offset, name) in output.footnote_references {
        document_with_footnotes += &output.document[current_offset..offset];
        current_offset = offset;
        document_with_footnotes += &format!(
            r#"<label for="sidenote-{name}" class="margin-toggle sidenote-number"></label><input type="checkbox" id="sidenote-{name}" class="margin-toggle"/>"#,
        );
        if !already_seen.contains(&name) {
            document_with_footnotes += r#"<span class="sidenote">"#;
            document_with_footnotes += &output.footnotes[&name];
            document_with_footnotes += r#"</span>"#;
            already_seen.insert(name);
        }
    }
    document_with_footnotes += &output.document[current_offset..];

    print!("{document_with_footnotes}");

    Ok(())
}
