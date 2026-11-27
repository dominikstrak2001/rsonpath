use rsonpath_tree_automaton::{engine::NaiveEngine, query, tree::JsonNode, Error};
use serde::Deserialize;
use serde_json::Value;
use std::{fs, path::Path};
use test_case::test_case;

const BOOKSTORE: &str = r#"
{
  "store": {
    "book": [
      { "category": "reference", "author": "Thomas H. Cormen", "title": "Introduction to Algorithms", "price": 89.99 },
      { "category": "reference", "author": "Martin Kleppmann", "title": "Designing Data-Intensive Applications", "price": 45.5 },
      { "category": "fiction", "author": "Veronica Roth", "title": "Divergent", "series": "Divergent", "price": 9.99 },
      { "category": "fiction", "author": "Remigiusz Mróz", "title": "Kasacja", "series": "Joanna Chyłka", "price": 7.99 },
      { "category": "fiction", "author": "J. R. R. Tolkien", "title": "The Lord of the Rings", "price": 22.99 }
    ],
    "bicycle": { "color": "red", "price": 399 }
  }
}"#;

const CLRS: &str = r#"{ "category": "reference", "author": "Thomas H. Cormen", "title": "Introduction to Algorithms", "price": 89.99 }"#;
const DDIA: &str = r#"{ "category": "reference", "author": "Martin Kleppmann", "title": "Designing Data-Intensive Applications", "price": 45.5 }"#;
const DIVERGENT: &str = r#"{ "category": "fiction", "author": "Veronica Roth", "title": "Divergent", "series": "Divergent", "price": 9.99 }"#;
const KASACJA: &str = r#"{ "category": "fiction", "author": "Remigiusz Mróz", "title": "Kasacja", "series": "Joanna Chyłka", "price": 7.99 }"#;
const LOTR: &str =
    r#"{ "category": "fiction", "author": "J. R. R. Tolkien", "title": "The Lord of the Rings", "price": 22.99 }"#;
const BICYCLE: &str = r#"{ "color": "red", "price": 399 }"#;
const AUTHORS: &[&str] = &[
    r#""Thomas H. Cormen""#,
    r#""Martin Kleppmann""#,
    r#""Veronica Roth""#,
    r#""Remigiusz Mróz""#,
    r#""J. R. R. Tolkien""#,
];
const AUTHORS_AND_TITLES: &[&str] = &[
    r#""Thomas H. Cormen""#,
    r#""Martin Kleppmann""#,
    r#""Veronica Roth""#,
    r#""Remigiusz Mróz""#,
    r#""J. R. R. Tolkien""#,
    r#""Introduction to Algorithms""#,
    r#""Designing Data-Intensive Applications""#,
    r#""Divergent""#,
    r#""Kasacja""#,
    r#""The Lord of the Rings""#,
];

fn run(document: &str, query_str: &str) -> Result<Vec<String>, Error> {
    let value: Value = serde_json::from_str(document).expect("test document must be valid JSON");
    run_on_tree(&JsonNode::from_json(&value), query_str)
}

fn run_on_tree(tree: &JsonNode, query_str: &str) -> Result<Vec<String>, Error> {
    let mut results: Vec<String> = query(query_str, tree, &NaiveEngine)?
        .into_iter()
        .map(|node| node.to_json().to_string())
        .collect();
    results.sort();
    Ok(results)
}

/// Results are compared as sorted lists, because their order is unspecified.
fn normalize(expected: &[&str]) -> Vec<String> {
    let mut expected: Vec<String> = expected
        .iter()
        .map(|json| {
            serde_json::from_str::<Value>(json)
                .expect("expected node must be valid JSON")
                .to_string()
        })
        .collect();
    expected.sort();
    expected
}

#[test_case("$.store.book[*].author", AUTHORS; "child authors")]
#[test_case("$..author", AUTHORS; "descendant authors")]
#[test_case("$.store.*", &[BICYCLE, &format!("[{CLRS},{DDIA},{DIVERGENT},{KASACJA},{LOTR}]")]; "store members")]
#[test_case("$.store..price", &["89.99", "45.5", "9.99", "7.99", "22.99", "399"]; "all prices")]
#[test_case("$..book[2]", &[DIVERGENT]; "third book")]
#[test_case("$..book[-1]", &[LOTR]; "last book")]
#[test_case("$..book[0,1]", &[CLRS, DDIA]; "union of indices")]
#[test_case("$..book[:2]", &[CLRS, DDIA]; "slice")]
#[test_case("$..book[::-2]", &[LOTR, DIVERGENT, CLRS]; "backward slice")]
#[test_case("$.store.book[1:3].title", &[r#""Designing Data-Intensive Applications""#, r#""Divergent""#]; "titles of a slice")]
#[test_case("$..book[*]['author','title']", AUTHORS_AND_TITLES; "union of names")]
#[test_case("$..series", &[r#""Divergent""#, r#""Joanna Chyłka""#]; "optional member")]
#[test_case("$..[0]", &[CLRS]; "first element of any array")]
#[test_case("$", &[BOOKSTORE]; "root")]
#[test_case("$.author", &[]; "child step does not skip levels")]
#[test_case("$.store.book.author", &[]; "name selector does not match array elements")]
fn bookstore(query_str: &str, expected: &[&str]) {
    let results = run(BOOKSTORE, query_str).expect("query must compile");
    assert_eq!(results, normalize(expected), "query {query_str}");
}

#[test]
fn bookstore_all_descendants() {
    let results = run(BOOKSTORE, "$..*").expect("query must compile");
    assert_eq!(results.len(), 32, "every node except the root");
}

#[test_case("$.store.book[?@.price < 10]", &[DIVERGENT, KASACJA]; "comparison with literal")]
#[test_case("$..book[?@.series]", &[DIVERGENT, KASACJA]; "existence")]
#[test_case("$.store.book[?@.category == 'fiction' && @.price > 10]", &[LOTR]; "conjunction")]
#[test_case("$.store.book[?!(@.category == 'fiction')]", &[CLRS, DDIA]; "negation")]
#[test_case("$.store.book[?@.series == @.title]", &[DIVERGENT]; "two queries compared")]
#[test_case("$.store.book[?@.series != @.title]", &[CLRS, DDIA, KASACJA, LOTR]; "missing value differs from a present one")]
#[test_case("$.store.book[?@.foo == @.bar]", &[CLRS, DDIA, DIVERGENT, KASACJA, LOTR]; "two missing values are equal")]
#[test_case("$.store.book[?@.price < $.store.book[1].price]", &[DIVERGENT, KASACJA, LOTR]; "absolute query in comparison")]
#[test_case("$..[?@.color == 'red']", &[BICYCLE]; "descendant filter")]
#[test_case("$.store[?@[?@.price > 50]]", &[&format!("[{CLRS},{DDIA},{DIVERGENT},{KASACJA},{LOTR}]")]; "nested filter")]
fn bookstore_filters(query_str: &str, expected: &[&str]) {
    let results = run(BOOKSTORE, query_str).expect("query must compile");
    assert_eq!(results, normalize(expected), "query {query_str}");
}

#[derive(Deserialize)]
struct TestFile {
    input: TestInput,
    queries: Vec<TestCase>,
}

#[derive(Deserialize)]
struct TestInput {
    source: TestSource,
}

#[derive(Deserialize)]
struct TestSource {
    json_string: Option<String>,
}

#[derive(Deserialize)]
struct TestCase {
    query: String,
    disabled: Option<toml::Value>,
    results: ExpectedResults,
}

#[derive(Deserialize)]
struct ExpectedResults {
    count: usize,
    nodes: Option<Vec<String>>,
}

/// Runs the test cases from rsonpath-test's TOML documents.
/// Documents stored in separate large files are skipped, as the naive engine is quadratic.
#[test]
fn rsonpath_test_cases() {
    let test_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../rsonpath-test/documents/toml");
    let mut paths: Vec<_> = fs::read_dir(&test_dir)
        .expect("test directory must exist")
        .map(|entry| entry.expect("test file entry must be readable").path())
        .collect();
    paths.sort();

    let mut checked = 0;
    let mut failures = vec![];
    let mut skipped = vec![];
    for path in paths {
        let file: TestFile = toml::from_str(&fs::read_to_string(&path).expect("test file must be readable"))
            .expect("test file must parse");
        let Some(document) = file.input.source.json_string else {
            continue;
        };
        let name = path.file_name().expect("test path has a file name").to_string_lossy();
        let cases: Vec<_> = file.queries.iter().filter(|case| case.disabled.is_none()).collect();

        // rsonpath does not validate its input, so some documents are not valid JSON.
        let tree = match serde_json::from_str::<Value>(&document) {
            Ok(value) => JsonNode::from_json(&value),
            Err(err) => {
                skipped.push(format!("{name} ({} queries): {err}", cases.len()));
                continue;
            }
        };

        for case in cases {
            checked += 1;
            match run_on_tree(&tree, &case.query) {
                Err(err) => failures.push(format!("{name}: {} failed: {err}", case.query)),
                Ok(results) if results.len() != case.results.count => failures.push(format!(
                    "{name}: {} returned {} nodes, expected {}",
                    case.query,
                    results.len(),
                    case.results.count
                )),
                Ok(results) => {
                    if let Some(nodes) = &case.results.nodes {
                        let expected = normalize(&nodes.iter().map(String::as_str).collect::<Vec<_>>());
                        if results != expected {
                            failures.push(format!("{name}: {} returned different nodes", case.query));
                        }
                    }
                }
            }
        }
    }

    println!("checked {checked} test cases, skipped documents that are not valid JSON:");
    for line in &skipped {
        println!("  {line}");
    }
    assert!(checked > 0, "no test cases found in {}", test_dir.display());
    assert!(
        failures.is_empty(),
        "{} of {checked} test cases failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
