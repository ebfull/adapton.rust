#[deriving(Clone,Show,Hash)]
enum NameCon { 
    Symbol (Box<String>),
    ForkL (Box<Name>),
    ForkR (Box<Name>),
    Pair (Box<Name>,Box<Name>),
}
pub type Name = NameCon;
#[allow(dead_code)]
pub fn symbol (s:String) -> Name {
    NameCon::Symbol(box s)
}
#[allow(dead_code)]
pub fn fork (n:Name) -> (Name,Name) {
    let m = n.clone () ;
    (NameCon::ForkL(box n), NameCon::ForkR(box m))
}    
#[allow(dead_code)]
pub fn pair (n:Name, m:Name) -> Name {
    NameCon::Pair(box n, box m)
}
#[allow(dead_code)]
pub fn printstuff () {
    let n = symbol (format!("one")) ;
    let m = symbol (format!("two")) ;
    println!("Adapton: {}, {}", n, m);
    println!("Adapton: {}, {}", fork(n), m);
}
#[test] 
fn test_printstuff () {
    printstuff();
}