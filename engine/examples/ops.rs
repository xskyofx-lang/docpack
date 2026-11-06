// Temporary probe: counts content operators of one page.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let doc = lopdf::Document::load(&a[1]).unwrap();
    let pages = doc.get_pages();
    let id = pages[&a[2].parse::<u32>().unwrap()];
    let content = doc.get_and_decode_page_content(id).unwrap();
    let mut counts = std::collections::BTreeMap::new();
    for op in &content.operations { *counts.entry(op.operator.clone()).or_insert(0) += 1; }
    println!("{counts:?}");
    for op in content.operations.iter().filter(|o| ["re","f","f*","S","l","m","h","B","n","W","cm","Do"].contains(&o.operator.as_str())).take(40) { println!("{} {:?}", op.operator, op.operands); }
}
