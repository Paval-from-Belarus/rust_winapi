extern crate embed_resource;
fn main() {
    embed_resource::compile("resources/resources.rc", embed_resource::NONE);
    println!("Resource loaded");
}