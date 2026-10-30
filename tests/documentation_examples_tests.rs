// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

const DOCUMENTS: &[(&str, &str)] = &[
    ("README.md", include_str!("../README.md")),
    ("README.zh_CN.md", include_str!("../README.zh_CN.md")),
    ("doc/user_guide.md", include_str!("../doc/user_guide.md")),
    ("doc/user_guide.zh_CN.md", include_str!("../doc/user_guide.zh_CN.md")),
    ("doc/json_design.md", include_str!("../doc/json_design.md")),
    ("doc/json_design.zh_CN.md", include_str!("../doc/json_design.zh_CN.md")),
    ("doc/number_contract.md", include_str!("../doc/number_contract.md")),
    (
        "doc/number_contract.zh_CN.md",
        include_str!("../doc/number_contract.zh_CN.md"),
    ),
    (
        "doc/benchmark_baseline.md",
        include_str!("../doc/benchmark_baseline.md"),
    ),
    (
        "doc/benchmark_baseline.zh_CN.md",
        include_str!("../doc/benchmark_baseline.zh_CN.md"),
    ),
];

#[derive(Debug)]
struct RustBlock {
    fence_line: usize,
    ignored: bool,
    body: String,
}

#[derive(Debug)]
struct RustScan {
    blocks: Vec<RustBlock>,
    errors: Vec<String>,
}

fn rust_blocks(markdown: &str, document: &str) -> RustScan {
    let mut blocks = Vec::new();
    let mut errors = Vec::new();
    let mut active: Option<(usize, bool, String)> = None;

    for (index, line) in markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        if let Some((start_line, ignored, body)) = active.as_mut() {
            if trimmed.starts_with("```") {
                blocks.push(RustBlock {
                    fence_line: *start_line,
                    ignored: *ignored,
                    body: std::mem::take(body),
                });
                active = None;
            } else {
                body.push_str(line);
                body.push('\n');
            }
        } else if let Some(info) = trimmed.strip_prefix("```") {
            let language = info.trim();
            if language == "rust" || language.starts_with("rust,") {
                active = Some((
                    index + 1,
                    language.split(',').any(|part| part == "ignore"),
                    String::new(),
                ));
            }
        }
    }

    if let Some((fence_line, _, _)) = active {
        errors.push(format!(
            "{document}: Rust fence opened at line {fence_line} is not closed"
        ));
    }
    RustScan { blocks, errors }
}

fn hidden_or_local_jcs(lines: &str) -> Option<(&str, usize)> {
    for (index, line) in lines.lines().enumerate() {
        let source = line.trim_start();
        if source == "#" || source.starts_with("# ") {
            return Some(("rustdoc hidden line", index + 1));
        }
        if source.contains("cfg(feature = \"jcs\")") || source.contains("cfg(not(feature = \"jcs\"))") {
            return Some(("consumer-local jcs cfg", index + 1));
        }
    }
    None
}

fn inspect_document(name: &str, markdown: &str) -> Vec<String> {
    let scan = rust_blocks(markdown, name);
    let mut errors = scan.errors;
    for block in scan.blocks {
        if let Some((problem, body_line)) = hidden_or_local_jcs(&block.body) {
            errors.push(format!(
                "{name}: Rust fence at line {} contains {problem} on line {}",
                block.fence_line,
                block.fence_line + body_line
            ));
        }
        let jcs_example = block.body.contains("Jcs") || block.body.contains("jcs");
        if jcs_example && !block.ignored {
            errors.push(format!(
                "{name}: JCS Rust fence at line {} must use rust,ignore",
                block.fence_line
            ));
        }
        if block.ignored && !jcs_example {
            errors.push(format!(
                "{name}: ignored Rust fence at line {} is not a JCS example",
                block.fence_line
            ));
        }
    }
    errors
}

#[test]
fn published_markdown_rust_fences_are_copyable_and_jcs_is_explicit() {
    let mut errors = Vec::new();
    for (name, markdown) in DOCUMENTS {
        errors.extend(inspect_document(name, markdown));
        if (name.starts_with("README") || name.contains("user_guide")) && !markdown.contains("--include-ignored") {
            errors.push(format!("{name}: missing explicit ignored JCS doctest command"));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn markdown_scanner_checks_only_rust_fences_and_recognizes_attributes() {
    let markdown = "# title\n\n```text\n# comment\n```\n\n```rust\n#[derive(Debug)]\nfn main() {}\n```\n";
    assert!(inspect_document("fixture.md", markdown).is_empty());
}

#[test]
fn markdown_scanner_rejects_hidden_lines_and_local_jcs_wrappers() {
    let hidden = "```rust\nfn main() {\n# let x = 1;\n}\n```\n";
    assert!(inspect_document("fixture.md", hidden)[0].contains("line 3"));

    let cfg = "```rust\n#[cfg(feature = \"jcs\")]\nfn main() {}\n```\n";
    assert!(inspect_document("fixture.md", cfg)[0].contains("consumer-local jcs cfg"));
}

#[test]
fn markdown_scanner_recognizes_multiple_ignored_rust_fences() {
    let markdown = "```rust,ignore\nuse qubit_json::Jcs;\n```\n```rust\nfn main() {}\n```\n";
    let blocks = rust_blocks(markdown, "fixture.md").blocks;
    assert_eq!(blocks.len(), 2);
    assert!(blocks[0].ignored);
    assert!(!blocks[1].ignored);
}

#[test]
fn markdown_scanner_rejects_unclosed_fences_and_unignored_jcs_examples() {
    let unclosed = "```rust\nfn main() {}\n";
    assert!(inspect_document("fixture.md", unclosed)[0].contains("not closed"));

    let unignored_jcs = "```rust\nuse qubit_json::decode::JcsDecoder;\n```\n";
    assert!(inspect_document("fixture.md", unignored_jcs)[0].contains("must use rust,ignore"));
}
