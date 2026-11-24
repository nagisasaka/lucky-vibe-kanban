//! Git-only Markdown projection. Never mutate the adopted Issue/Wiki snapshot.
use std::ops::Range;

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd, TextMergeWithOffset};
use url::{Origin, Url};

pub fn public_origin(value: Option<&str>) -> Option<String> {
    let url = Url::parse(value?.trim()).ok()?;
    match url.origin() {
        Origin::Tuple(..) => Some(url.origin().ascii_serialization()),
        Origin::Opaque(_) => None,
    }
}

fn replace_internal(text: &str, origin: Option<&str>) -> String {
    let Some(origin) = origin.and_then(|s| Url::parse(s).ok()).map(|u| u.origin()) else {
        return text.to_owned();
    };
    let mut finder = linkify::LinkFinder::new();
    finder.kinds(&[linkify::LinkKind::Url]);
    let mut result = String::new();
    let mut cursor = 0;
    for link in finder.links(text) {
        if Url::parse(link.as_str()).is_ok_and(|url| url.origin() == origin) {
            result.push_str(&text[cursor..link.start()]);
            result.push_str("LVK 内部リンク");
            cursor = link.end();
        }
    }
    result.push_str(&text[cursor..]);
    result
}

pub(super) fn project(markdown: &str, origin: Option<&str>) -> String {
    let parser = Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );
    let mut edits: Vec<(Range<usize>, String)> = parser
        .reference_definitions()
        .iter()
        .map(|(_, definition)| (definition.span.clone(), String::new()))
        .collect();
    // A link/image's event range includes its destination; child event ranges
    // describe only its visible label, including emphasis/code source syntax.
    let mut labels: Vec<(Range<usize>, Option<Range<usize>>)> = Vec::new();
    for (event, range) in TextMergeWithOffset::new(parser.into_offset_iter()) {
        let close = matches!(event, Event::End(TagEnd::Link | TagEnd::Image));
        if close {
            if let Some((whole, label)) = labels.pop() {
                if let Some(label) = label {
                    edits.push((whole.start..label.start, String::new()));
                    edits.push((label.end..whole.end, String::new()));
                } else {
                    edits.push((whole, String::new()));
                }
            }
            continue;
        }
        if let Some((_, label)) = labels.last_mut() {
            match label {
                Some(label) => {
                    label.start = label.start.min(range.start);
                    label.end = label.end.max(range.end);
                }
                None => *label = Some(range.clone()),
            }
        }
        match event {
            Event::Start(Tag::Link {
                link_type: LinkType::Autolink | LinkType::Email,
                ..
            }) => {
                // Angle URLs are literal URLs: retain external ones, too.
                // A sentinel balances End(Link) without stripping their brackets.
                labels.push((range.clone(), Some(range)));
            }
            Event::Start(Tag::Link { link_type, .. } | Tag::Image { link_type, .. }) => {
                // pulldown-cmark's collapsed-reference range excludes the final [].
                let end = if link_type == LinkType::Collapsed
                    && markdown[range.end..].starts_with("[]")
                {
                    range.end + 2
                } else {
                    range.end
                };
                labels.push((range.start..end, None));
            }
            Event::Text(text) => {
                // Decode Markdown escapes/entities in labels before origin matching.
                // Otherwise e.g. https&#58;//internal.example could leak a URL.
                let raw = &markdown[range.clone()];
                let replaced = replace_internal(raw, origin);
                if replaced != raw {
                    edits.push((range, replaced));
                } else if !labels.is_empty() {
                    let replaced = replace_internal(&text, origin);
                    if replaced != text.as_ref() {
                        edits.push((range, replaced));
                    }
                }
            }
            _ => {}
        }
    }
    // The parser records only the first definition of a duplicate label. Mask
    // definitions already found, preserving offsets, to discover shadowed ones.
    let mut masked = markdown.as_bytes().to_vec();
    loop {
        let definitions: Vec<_> = Parser::new(std::str::from_utf8(&masked).unwrap())
            .reference_definitions()
            .iter()
            .map(|(_, d)| d.span.clone())
            .collect();
        if definitions.is_empty() {
            break;
        }
        for span in definitions {
            for byte in &mut masked[span.clone()] {
                if *byte != b'\n' && *byte != b'\r' {
                    *byte = b' ';
                }
            }
            edits.push((span, String::new()));
        }
    }
    edits.sort_by_key(|(range, _)| (range.start, range.end));
    let mut result = String::new();
    let mut cursor = 0;
    for (range, replacement) in edits {
        if range.start < cursor {
            continue;
        }
        result.push_str(&markdown[cursor..range.start]);
        result.push_str(&replacement);
        cursor = range.end;
    }
    result.push_str(&markdown[cursor..]);
    // Also cover literal URLs in code/HTML and in labels retained above.
    replace_internal(&result, origin)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_images_references_and_structure() {
        let input = "# 見出し\n\n> [**意味** `code`](https://external.test/a \"title\")\n- ![説明](https://external.test/image)\n- [参照][r] / [r][] / [r]\n\n[r]: https://external.test/ref \"説明\"\n[unused]: https://external.test/unused\n[r]: https://external.test/shadowed\n";
        let output = project(input, None);
        assert!(
            output.starts_with("# 見出し\n\n> **意味** `code`\n- 説明\n- 参照 / r / r\n"),
            "{output}"
        );
        assert!(!output.contains("https://"));
        assert_eq!(project(&output, None), output);
        assert_eq!(
            project(
                "[![alt](image.png)](target) ![**bold**][img]\n\n[img]: image.png",
                None
            )
            .trim(),
            "alt **bold**"
        );
    }

    #[test]
    fn html_and_code_preserve_literal_syntax_but_remove_internal_urls() {
        let input = "<a href=\"https://outside.test/a\">HTML</a>\n\n`[code](https://outside.test/code)`\n\n```text\n![literal](https://outside.test/image)\nhttps://lvk.example.test/private\n```\n";
        let output = project(input, Some("https://lvk.example.test"));
        assert_eq!(
            output,
            input.replace("https://lvk.example.test/private", "LVK 内部リンク")
        );
        assert_eq!(project(&output, Some("https://lvk.example.test")), output);
        assert_eq!(project(input, None), input);
    }

    #[test]
    fn legacy_comparison_uses_the_same_projection() {
        let origin = Some("https://lvk.example.test");
        let legacy = "[目的](https://lvk.example.test/old)\n";
        let adopted = "[目的](https://outside.test/new)\n";
        assert_eq!(project(legacy, origin), project(adopted, origin));
        assert_eq!(
            project(legacy, origin),
            project(&project(adopted, origin), origin)
        );
        assert_ne!(project(legacy, origin), project("本文変更", origin));
    }

    #[test]
    fn origins_literal_urls_and_labels() {
        let origin = public_origin(Some("https://LVK.example.test:443/base"));
        assert_eq!(origin.as_deref(), Some("https://lvk.example.test"));
        let input = "https://lvk.example.test/a <https://lvk.example.test/b> [https://lvk.example.test/c](https://external.test) [https&#58;//lvk.example.test/d](dest) `https://lvk.example.test/e`";
        let output = project(input, origin.as_deref());
        assert!(!output.contains("lvk.example.test"), "{output}");
        assert_eq!(output.matches("LVK 内部リンク").count(), 5);
        let external = "https://lvk.example.test.evil/a https://lvk.example.test:8443/a http://lvk.example.test/a https://elsewhere.test/a <https://elsewhere.test/b> /absolute/path Name";
        assert_eq!(project(external, origin.as_deref()), external);
        assert_eq!(
            project(
                "https://lvk.example.test/a <https://elsewhere.test/b>",
                None
            ),
            "https://lvk.example.test/a <https://elsewhere.test/b>"
        );
        assert_eq!(
            project(
                "https://lvk.example.test:8443/a",
                Some("https://lvk.example.test:8443")
            ),
            "LVK 内部リンク"
        );
        assert_eq!(project(&output, origin.as_deref()), output);
        assert_eq!(
            project("https://lvk.example.test:443/a", origin.as_deref()),
            "LVK 内部リンク"
        );
        assert_eq!(
            project("https://lvk.example.test@outside.test/a", origin.as_deref()),
            "https://lvk.example.test@outside.test/a"
        );
        assert_eq!(
            project(
                r"Name \* /absolute/path https://lvk.example.test/a",
                origin.as_deref()
            ),
            r"Name \* /absolute/path LVK 内部リンク"
        );
        assert_eq!(public_origin(None), None);
        assert_eq!(public_origin(Some("invalid")), None);
    }
}
