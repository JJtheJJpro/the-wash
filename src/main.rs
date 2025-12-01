mod tests;

fn main() {
    let h = the_wash::html::parse_html("a<os>");
    println!("{:#?}", h);
}
