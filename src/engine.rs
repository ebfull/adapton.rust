use std::fmt::Debug;
use std::collections::HashMap;
use std::mem::replace;
use std::mem::transmute;
use std::rc::Rc;
use std::fmt;
use std::marker::PhantomData;
use std::fmt::{Formatter,Result};
use std::hash::{Hash,Hasher};
use std::num::Zero;

use macros::*;
use adapton_sigs::*;

const engineMsgStr : &'static str = "adapton::engine:";

fn engineMsg (indent:Option<usize>) -> String {
    match indent {
        None => "adapton::engine:".to_string(),
        Some(indent) => {
            let mut indent_str = "".to_string() ;
            for i in 1..indent {
                indent_str.push_str("···〉")
            };
            ("adapton::engine:".to_string() + &indent_str)
    }}}

macro_rules! engineMsg {
    ( $st:expr ) => {{
        engineMsg(Some($st.stack.len()))
    }}
}

// Names provide a symbolic way to identify nodes.
#[derive(Hash,PartialEq,Eq,Clone)]
pub struct Name {
    hash : u64, // hash of symbol
    symbol : Rc<NameSym>,
}
impl Debug for Name {
    fn fmt(&self, f:&mut Formatter) -> Result { self.symbol.fmt(f) }
}

// Each location identifies a node in the DCG.
#[derive(Hash,PartialEq,Eq,Clone)]
pub struct Loc {
    hash : u64, // hash of (path,id)
    path : Rc<Path>,
    id   : Rc<ArtId<Name>>,
}
impl Debug for Loc {
    fn fmt(&self, f:&mut Formatter) -> Result { self.path.fmt(f) ; self.id.fmt(f) }
}

#[derive(Hash,Debug,PartialEq,Eq,Clone)]
enum ArtId<Name> {
    Structural(u64), // Identifies an Art::Loc based on hashing content.
    Nominal(Name),   // Identifies an Art::Loc based on a programmer-chosen name.
}

#[derive(Debug)]
pub struct Engine {
    root  : Rc<Loc>,
    table : HashMap<Rc<Loc>, Box<GraphNode>>,
    stack : Vec<Frame>,
    cnt   : Cnt,
}

impl Hash  for     Engine { fn hash<H>(&self, _state: &mut H) where H: Hasher { unimplemented!() }}
//impl Debug for     Engine { fn fmt(&self, _f:&mut Formatter) -> Result { unimplemented!() } }
impl Eq    for     Engine { }
impl PartialEq for Engine { fn eq(&self, _other:&Self) -> bool { unimplemented!() } }
impl Clone for     Engine { fn clone(&self) -> Self { unimplemented!() } }

// NameSyms: For a general semantics of symbols, see Chapter 31 of PFPL 2nd Edition. Harper 2015:
// http://www.cs.cmu.edu/~rwh/plbook/2nded.pdf
//
#[derive(Hash,Debug,PartialEq,Eq,Clone)]
enum NameSym {
    Root, // Root identifies the outside environment of Rust code.
    String(String), // Strings encode globally-unique symbols.
    Usize(usize),   // USizes encode globally-unique symbols.
    Pair(Rc<NameSym>,Rc<NameSym>), // A pair of unique symbols, interpeted as a symbol, is unique
    ForkL(Rc<NameSym>), // Left projection of a unique symbol is unique
    ForkR(Rc<NameSym>), // Right projection of a unique symbol is unique
    //Rc(Rc<NameSym>),
    //Nil,  // Nil for non-symbolic, hash-based names.
}

// Paths are built implicitly via the Adapton::ns command.
#[derive(Hash,Debug,PartialEq,Eq,Clone)]
enum Path {
    Empty,
    Child(Rc<Path>,Name),
}

// The DCG structure consists of `GraphNode`s:
trait GraphNode {
    fn preds_alloc<'r> (self:&'r mut Self) -> Vec<Rc<Loc>> ;
    fn preds_obs<'r>   (self:&'r mut Self) -> Vec<Rc<Loc>> ;
    fn preds_insert<'r>(self:&'r mut Self, Effect, &Rc<Loc>) -> () ;
    fn preds_remove<'r>(self:&'r mut Self, &Rc<Loc>) -> () ;
    fn succs_def<'r>   (self:&'r mut Self) -> bool ;
    fn succs_mut<'r>   (self:&'r mut Self) -> &'r mut Vec<Succ> ;
}

#[derive(Debug)]
struct Frame {
    loc   : Rc<Loc>,    // The currently-executing node
    path  : Rc<Path>,   // The current path for creating new nodes; invariant: (prefix-of frame.loc.path frame.path)
    succs : Vec<Succ>,  // The currently-executing node's effects (viz., the nodes it demands)
}

#[derive(Debug,Clone)]
struct Succ {
    effect : Effect,
    dep    : Rc<Box<EngineDep>>, // Abstracted dependency information (e.g., for Observe Effect, the prior observed value)
    loc    : Rc<Loc>, // Target of the effect, aka, the successor, by this edge
    dirty  : bool,    // mutated to dirty when loc changes, or any of its successors change
}

#[derive(PartialEq,Eq,Debug,Clone)]
enum Effect {
    Observe,
    Allocate,
}
struct EngineRes {
    changed : bool,
}
// EngineDep abstracts over the value produced by a dependency, as
// well as mechanisms to update and/or re-produce it.
trait EngineDep : Debug {
    fn change_prop (self:&Self, st:&mut Engine, loc:&Rc<Loc>) -> EngineRes ;
}


// ----------------------------------------------------------------------------------------------------

#[derive(Debug)]
struct NoDependency;
impl EngineDep for NoDependency {
    fn change_prop (self:&Self, _st:&mut Engine, _loc:&Rc<Loc>) -> EngineRes { EngineRes{changed:false} }
}

#[derive(Debug)]
struct AllocDependency<T> { val:T }
impl<T:Debug> EngineDep for AllocDependency<T> {
    fn change_prop (self:&Self, _st:&mut Engine, _loc:&Rc<Loc>) -> EngineRes { EngineRes{changed:true} } // TODO-Later: Make this a little better.
}


trait ShapeShifter {
    fn be_node<'r> (self:&'r mut Self) -> &'r mut Box<GraphNode> ;
}

impl fmt::Debug for GraphNode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(GraphNode)")
    }
}

// Structureful (Non-opaque) nodes:
#[allow(dead_code)] // Pure case: not introduced currently.
#[derive(Debug)]
enum Node<Res> {
    Comp(CompNode<Res>),
    Pure(PureNode<Res>),
    Mut(MutNode<Res>),
    Unused,
}

// PureNode<T> for pure hash-consing of T's.
// Location in table never changes value.
#[derive(Debug)]
struct PureNode<T> {
    val : T,
}

// MutNode<T> for mutable content of type T.
// The set operation mutates a MutNode; set may only be called by *outer* Rust environment.
// Its notable that the CompNodes' producers do not directly change the value of MutNodes with set.
// They may indirectly mutate these nodes by performing nominal allocation; mutation is limited to "one-shot" changes.
#[derive(Debug)]
struct MutNode<T> {
    preds : Vec<(Effect,Rc<Loc>)>,
    val   : T,
}

// CompNode<Res> for a suspended computation whose resulting value of
// type T.  The result of the CompNode is affected in two ways: the
// (1) producer may change, which may affect the result and (2) the
// values produced by the successors may change, indirectly
// influencing how the producer produces its resulting value.
struct CompNode<Res> {
    preds    : Vec<(Effect, Rc<Loc>)>,
    succs    : Vec<Succ>,
    producer : Box<Producer<Res>>, // Producer can be App<Arg,Res>, where type Arg is hidden.
    res      : Option<Res>,
}
// Produce a value of type Res.
trait Producer<Res> : Debug {
    fn produce(self:&Self, st:&mut Engine) -> Res;
    fn copy(self:&Self) -> Box<Producer<Res>>;
    fn eq(self:&Self, other:&Producer<Res>) -> bool;
    fn prog_pt<'r>(self:&'r Self) -> &'r ProgPt;
}
// Consume a value of type Arg.
trait Consumer<Arg> : Debug {
    fn consume(self:&mut Self, Arg);
    fn get_arg(self:&mut Self) -> Arg;
}
// struct App is hidden by traits Comp<Res> and CompWithArg<Res>, below.
#[derive(Clone)]
struct App<Arg:Debug,Spurious,Res> {
    prog_pt: ProgPt,
    fn_box:   Rc<Box<Fn(&mut Engine, Arg, Spurious) -> Res>>,
    arg:      Arg,
    spurious: Spurious,
}

// ---------- App implementation of Debug and Hash

impl<Arg:Debug,Spurious,Res> Debug for App<Arg,Spurious,Res> {
    fn fmt(&self, f: &mut Formatter) -> Result { self.prog_pt.fmt(f) ; self.arg.fmt(f) }
}

impl<Arg:Hash+Debug,Spurious,Res> Hash for App<Arg,Spurious,Res> {
    fn hash<H>(&self, state: &mut H) where H: Hasher { (&self.prog_pt,&self.arg).hash(state) }
}

// ---------- App implementation of Producer and Consumer traits:

impl<Arg:'static+PartialEq+Eq+Clone+Debug,Spurious:'static+Clone,Res:'static+Debug> Producer<Res>
    for App<Arg,Spurious,Res>
{
    fn produce(self:&Self, st:&mut Engine) -> Res {
        let f = self.fn_box.clone() ;
        st.cnt.eval += 1 ;
        info!("{} producer begin: ({:?} {:?})", engineMsg!(st), &self.prog_pt, &self.arg);
        let res = f (st,self.arg.clone(),self.spurious.clone()) ;
        info!("{} producer end: ({:?} {:?}) produces {:?}", engineMsg!(st), &self.prog_pt, &self.arg, &res);
        res
    }
    fn copy(self:&Self) -> Box<Producer<Res>> {
        Box::new(App{
            prog_pt:self.prog_pt.clone(),
            fn_box:self.fn_box.clone(),
            arg:self.arg.clone(),
            spurious:self.spurious.clone(),
        })
    }
    fn prog_pt<'r>(self:&'r Self) -> &'r ProgPt {
        & self.prog_pt
    }
    fn eq (&self, other:&Producer<Res>) -> bool {
        if &self.prog_pt == other.prog_pt() {
            let other = Box::new(other) ;
            // This is safe if the prog_pt implies unique Arg and Res types.
            let other : &Box<App<Arg,Spurious,Res>> = unsafe { transmute::<_,_>( other ) } ;
            self.arg == other.arg
        } else {
            false
        }
    }
}
impl<Arg:Clone+PartialEq+Eq+Debug,Spurious,Res> Consumer<Arg> for App<Arg,Spurious,Res> {
    fn consume(self:&mut Self, arg:Arg) { replace(&mut self.arg, arg); }
    fn get_arg(self:&mut Self) -> Arg   { self.arg.clone() }
}

// ----------- Location resolution:

fn lookup_abs<'r>(st:&'r mut Engine, loc:&Rc<Loc>) -> &'r mut Box<GraphNode> {
    match st.table.get_mut( loc ) {
        None => panic!("dangling pointer: {:?}", loc),
        Some(node) => node.be_node() // This is a weird workaround; TODO-Later: Investigate.
    }
}

// This only is safe in contexts where the type of loc is known.
// Unintended double-uses of names and hashes will generally cause uncaught type errors.
fn res_node_of_loc<'r,Res> (st:&'r mut Engine, loc:&Rc<Loc>) -> &'r mut Box<Node<Res>> {
    let abs_node = lookup_abs(st, loc) ;
    unsafe { transmute::<_,_>(abs_node) }
}

// ---------- Node implementation:

impl <Res> GraphNode for Node<Res> {
    fn preds_alloc<'r>(self:&'r mut Self) -> Vec<Rc<Loc>> {
        match *self { Node::Mut(ref mut nd) => nd.preds.iter().filter_map(|&(ref effect,ref loc)| if effect == &Effect::Allocate { Some(loc.clone()) } else { None } ).collect::<Vec<_>>(),
                      Node::Comp(ref mut nd) => nd.preds.iter().filter_map(|&(ref effect,ref loc)| if effect == &Effect::Allocate { Some(loc.clone()) } else { None } ).collect::<Vec<_>>(),
                      Node::Pure(_) => unreachable!(),
                      _ => unreachable!(),
        }}
                      
    fn preds_obs<'r>(self:&'r mut Self) -> Vec<Rc<Loc>> {
        match *self { Node::Mut(ref mut nd) => nd.preds.iter().filter_map(|&(ref effect,ref loc)| if effect == &Effect::Observe { Some(loc.clone()) } else { None } ).collect::<Vec<_>>(),
                      Node::Comp(ref mut nd) => nd.preds.iter().filter_map(|&(ref effect,ref loc)| if effect == &Effect::Observe { Some(loc.clone()) } else { None } ).collect::<Vec<_>>(),
                      Node::Pure(_) => unreachable!(),
                      _ => unreachable!(),
        }}
    fn preds_insert (self:&mut Self, eff:Effect, loc:&Rc<Loc>) -> () {
        match *self { Node::Mut(ref mut nd) => nd.preds.push ((eff,loc.clone())),
                      Node::Comp(ref mut nd) => nd.preds.push ((eff,loc.clone())),
                      Node::Pure(_) => unreachable!(),
                      _ => unreachable!(),
        }}
    fn preds_remove (self:&mut Self, loc:&Rc<Loc>) -> () {
        match *self { Node::Mut(ref mut nd) => nd.preds.retain (|eff_pred|{ let (_,ref pred) = *eff_pred; *pred != *loc }),
                      Node::Comp(ref mut nd) => nd.preds.retain (|eff_pred|{ let (_, ref pred) = *eff_pred; *pred != *loc}),
                      Node::Pure(_) => unreachable!(),
                      _ => unreachable!(),
        }}
    fn succs_def<'r>(self:&'r mut Self) -> bool {
        match *self { Node::Comp(_) => true, _ => false
        }}
    fn succs_mut<'r>(self:&'r mut Self) -> &'r mut Vec<Succ> {
        match *self { Node::Comp(ref mut n) => &mut n.succs,
                     _ => panic!("undefined"),
        }
    }
}

impl <Res> ShapeShifter for Box<Node<Res>> {
    fn be_node<'r>(self:&'r mut Self) -> &'r mut Box<GraphNode> {
        // TODO-Later: Why is this transmute needed here ??
        unsafe { transmute::<_,_>(self) }
    }
}

impl ShapeShifter for Box<GraphNode> {
    fn be_node<'r>(self:&'r mut Self) -> &'r mut Box<GraphNode> {
        // TODO-Later: Why is this transmute needed here ??
        unsafe { transmute::<_,_>(self) }
    }
}



impl<Res> fmt::Debug for CompNode<Res> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(CompNode)")
    }
}

// Performs the computation at loc, produces a result of type Res.
// Error if loc is not a Node::Comp.
fn produce<Res:'static+Debug+PartialEq+Eq+Clone>(st:&mut Engine, loc:&Rc<Loc>) -> Res
{
    info!("{} produce begin: {:?}", engineMsg!(st), &loc);
    let succs : Vec<Succ> = {
        let succs : Vec<Succ> = Vec::new();
        let node : &mut Node<Res> = res_node_of_loc( st, loc ) ;
        replace(node.succs_mut(), succs)
    } ;
    revoke_succs( st, loc, &succs );
    st.stack.push ( Frame{loc:loc.clone(),
                          path:loc.path.clone(),
                          succs:Vec::new(), } );
    let producer : Box<Producer<Res>> = {
        let node : &mut Node<Res> = res_node_of_loc( st, loc ) ;
        match *node {
            Node::Comp(ref nd) => nd.producer.copy(),
            _ => panic!("internal error"),
        }
    } ;
    let res = producer.produce( st ) ;
    let frame = match st.stack.pop() {
        None => panic!("expected Some _: stack invariants are broken"),
        Some(frame) => frame
    } ;
    assert!( &frame.loc == loc );
    for succ in &frame.succs {
        info!("{} produce: edge: {:?} --{:?}--dirty?:{:?}--> {:?}", engineMsg!(st), &loc, &succ.effect, &succ.dirty, &succ.loc);
        if succ.dirty {
            // This case witnesses an illegal use of nominal side effects
            panic!("invariants broken: newly-built DCG edge should be clean, but is dirty.")
        } ;
        let succ_loc = lookup_abs( st, &succ.loc );
        succ_loc.preds_insert( succ.effect.clone(), loc );
    } ;
    {
        let node : &mut Node<Res> = res_node_of_loc( st, loc ) ;
        match *node {
            Node::Comp(ref mut node) => {
                replace(&mut node.succs, frame.succs) ;
                replace(&mut node.res, Some(res.clone()))
            },
            _ => panic!("internal error"),
        }
    } ;
    match st.stack.last_mut() { None => (), Some(frame) => {
        let succ = Succ{loc:loc.clone(),
                        effect:Effect::Observe,
                        dep:Rc::new(Box::new(ProducerDep{res:res.clone()})),
                        dirty:false};
        frame.succs.push(succ)
    }};
    info!("{} produce end: {:?} produces {:?}", engineMsg!(st), &loc, &res);
    res
}

fn re_produce<Res:'static+Debug+PartialEq+Eq+Clone>(dep:&ProducerDep<Res>, st:&mut Engine, loc:&Rc<Loc>) -> EngineRes {
    let result : Res = produce( st, loc ) ;
    let changed = result != dep.res ;
    EngineRes{changed:changed}
}


// ---------- EngineDep implementation:

#[derive(Debug)]
struct ProducerDep<T> { res:T }

impl <Res:'static+Sized+Debug+PartialEq+Eq+Clone>
    EngineDep for ProducerDep<Res>
{
    fn change_prop(self:&Self, st:&mut Engine, loc:&Rc<Loc>) -> EngineRes {
        let stackLen = st.stack.len() ;
        info!("{} change_prop begin: {:?}", engineMsg!(st), loc);
        st.cnt.change_prop += 1 ;
        { // Handle cases where there is no internal computation to re-compute:
            let node : &mut Node<Res> = res_node_of_loc(st, loc) ;
            match *node {
                Node::Comp(_) => (),
                Node::Pure(_) => {
                    info!("{} change_prop early end: {:?} is Pure(_)", engineMsg(Some(stackLen)), loc);
                    return EngineRes{changed:false}
                },
                Node::Mut(ref nd) => {
                    info!("{} change_prop early end: {:?} is Mut(_)", engineMsg(Some(stackLen)), loc);
                    return EngineRes{changed:nd.val != self.res}
                },
                _ => panic!("undefined")
            }
        };
        let succs = {
            let node : &mut Node<Res> = res_node_of_loc(st, loc) ;
            assert!( node.succs_def() );
            node.succs_mut().clone()
        } ;
        for succ in succs.iter() {
            if succ.dirty {
                let dep = & succ.dep ;
                let res = dep.change_prop(st, &succ.loc) ;
                if res.changed {
                    info!("{} change_prop end (1/2): {:?} has a changed succ dependency: {:?}. Begin re-production:", engineMsg!(st), loc, &succ.loc);
                    let res = re_produce (self, st, loc);
                    info!("{} change_prop end (2/2): {:?} has a changed succ dependency: {:?}. End re-production.", engineMsg!(st), loc, &succ.loc);
                    return res
                }
            }
        } ;
        info!("{} change_prop end: {:?} is clean.", engineMsg!(st), &loc);
        // No early return =>
        //   all immediate dependencies are change-free:
        EngineRes{changed:false}
    }
}

// ---------- Node implementation:

fn revoke_succs<'x> (st:&mut Engine, src:&Rc<Loc>, succs:&Vec<Succ>) {
    for succ in succs.iter() {
        let succ_node : &mut Box<GraphNode> = lookup_abs(st, &succ.loc) ;
        succ_node.preds_remove(src)
    }
}

fn loc_of_id(path:Rc<Path>,id:Rc<ArtId<Name>>) -> Rc<Loc> {
    let hash = my_hash(&(&path,&id));
    Rc::new(Loc{path:path,id:id,hash:hash})
}

// Implement "sharing" of the dirty bit.
// The succ edge is returned as a mutable borrow, to permit checking
// and mutating the dirty bit.
fn get_succ_mut<'r>(st:&'r mut Engine, src_loc:&Rc<Loc>, eff:Effect, tgt_loc:&Rc<Loc>) -> &'r mut Succ {
    let stackLen = st.stack.len() ;
    let nd = lookup_abs( st, src_loc );
    info!("{} get_succ_mut: resolving {:?} --{:?}--dirty:?--> {:?}", engineMsg(Some(stackLen)), &src_loc, &eff, &tgt_loc);
    for succ in nd.succs_mut().iter_mut() {
        if (succ.effect == eff) && (&succ.loc == tgt_loc) {
            info!("{} get_succ_mut: resolved {:?} --{:?}--dirty:{:?}--> {:?}", engineMsg(Some(stackLen)), &src_loc, &succ.effect, &succ.dirty, &tgt_loc);
            return succ
        } else {}
    } ;
    panic!("tgt_loc is dangling in src_node.dem_succs")
}

fn dirty_pred_observers(st:&mut Engine, loc:&Rc<Loc>) {
    info!("{} dirty_pred_observers: {:?}", engineMsg!(st), loc);
    st.cnt.dirty += 1 ;
    let stackLen = st.stack.len() ;
    let pred_locs : Vec<Rc<Loc>> = lookup_abs( st, loc ).preds_obs() ;
    for pred_loc in pred_locs {
        if st.root.eq (&pred_loc) { panic!("root in preds") }
        else {
            let stop : bool = {
                // The stop bit communicates information from st for use below.
                info!("{} dirty_pred_observers: edge {:?} --> {:?} ...", engineMsg(Some(stackLen)), &pred_loc, &loc);
                let succ = get_succ_mut(st, &pred_loc, Effect::Observe, &loc) ;
                if succ.dirty { true } else {
                    replace(&mut succ.dirty, true);
                    info!("{} dirty_pred_observers: edge marked dirty: {:?} --{:?}--dirty:{:?}--> {:?}", engineMsg(Some(stackLen)), &pred_loc, &succ.effect, &succ.dirty, &loc);
                    false
                }} ;
            if !stop {
                dirty_pred_observers(st,&pred_loc);
            } else { info!("{} dirty_pred_observers: already dirty", engineMsg(Some(stackLen))) }
        }
    }
}

fn dirty_alloc(st:&mut Engine, loc:&Rc<Loc>) {
    info!("{} dirty_alloc: {:?}", engineMsg!(st), loc);
    dirty_pred_observers(st, loc);
    let stackLen = st.stack.len() ;
    let pred_locs : Vec<Rc<Loc>> = lookup_abs(st, loc).preds_alloc() ;
    for pred_loc in pred_locs {
        if st.root.eq (&pred_loc) { panic!("root in preds") }
        else {
            let stop : bool = {
                // The stop bit communicates information from st for use below.
                info!("{} dirty_alloc: edge {:?} --> {:?} ...", engineMsg(Some(stackLen)), &pred_loc, &loc);
                let succ = get_succ_mut(st, &pred_loc, Effect::Allocate, &loc) ;
                if succ.dirty { true } else {
                    info!("{} dirty_alloc: edge {:?} --> {:?} marked dirty", engineMsg(Some(stackLen)), &pred_loc, &loc);
                    replace(&mut succ.dirty, true);
                    false
                }} ;
            if !stop {
                dirty_pred_observers(st,&pred_loc);
            } else { info!("{} dirty_alloc: early stop", engineMsg(Some(stackLen))) }
        }
    }
}

fn do_set<T:Eq+Debug> (st:&mut Engine, cell:MutArt<T,Loc>, val:T) {
    info!("{} do_set: {:?} <--- {:?}", engineMsg!(st), cell, val);
    let changed : bool = {
        let node = res_node_of_loc( st, &cell.loc ) ;
        match **node {
            Node::Mut(ref mut nd) => {
                if nd.val == val {
                    false
                } else {
                    replace(&mut nd.val, val) ;
                    true
                }},
            _ => unreachable!(),
        }} ;
    if changed {
        dirty_alloc(st, &cell.loc)
    }
    else { }
}

impl Adapton for Engine {
    type Name = Name;
    type Loc  = Loc;

    fn new () -> Engine {
        let root = {
            let path   = Rc::new(Path::Empty);
            let symbol = Rc::new(NameSym::Root);
            let hash   = my_hash(&symbol);
            let name   = Name{symbol:symbol,hash:hash};
            let id     = Rc::new(ArtId::Nominal(name));
            let hash   = my_hash(&(&path,&id));
            let loc    = Rc::new(Loc{path:path.clone(),id:id,hash:hash});
            loc
        } ;
        let mut stack = Vec::new() ;
        stack.push( Frame{loc:root.clone(),
                          path:root.path.clone(),
                          succs:Vec::new()} ) ;
        Engine {
            root  : root,
            table : HashMap::new (),
            stack : stack,
            cnt   : Cnt::zero (),
        }
    }

    fn name_of_string (self:&mut Engine, sym:String) -> Name {
        let h = my_hash(&sym);
        let s = NameSym::String(sym) ;
        Name{ hash:h, symbol:Rc::new(s) }
    }

    fn name_of_usize (self:&mut Engine, sym:usize) -> Name {
        let h = my_hash(&sym) ;
        let s = NameSym::Usize(sym) ;
        Name{ hash:h, symbol:Rc::new(s) }
    }

    fn name_pair (self: &mut Engine, fst: Name, snd: Name) -> Name {
        let h = my_hash( &(fst.hash,snd.hash) ) ;
        let p = NameSym::Pair(fst.symbol, snd.symbol) ;
        Name{ hash:h, symbol:Rc::new(p) }
    }

    fn name_fork (self:&mut Engine, nm:Name) -> (Name, Name) {
        let h1 = my_hash( &(&nm, 11111111) ) ; // TODO-Later: make this hashing better.
        let h2 = my_hash( &(&nm, 22222222) ) ;
        ( Name{ hash:h1,
                symbol:Rc::new(NameSym::ForkL(nm.symbol.clone())) } ,
          Name{ hash:h2,
                symbol:Rc::new(NameSym::ForkR(nm.symbol)) } )
    }

    fn ns<T,F> (self: &mut Self, nm:Name, body:F) -> T where F:FnOnce(&mut Self) -> T {
        let path = match self.stack.last() { None => unreachable!(), Some(frame) => frame.path.clone() } ;
        let path_body = Rc::new(Path::Child(path, nm)) ;
        let path_pre = match self.stack.last_mut() { None => unreachable!(), Some(frame) => replace(&mut frame.path, path_body) } ;
        let x = body(self) ;
        let path_body = match self.stack.last_mut() { None => unreachable!(), Some(frame) => replace(&mut frame.path, path_pre) } ;
        drop(path_body);
        x
    }

    fn cnt<Res,F> (self: &mut Self, body:F) -> (Res,Cnt)
        where F:FnOnce(&mut Self) -> Res
    {
        let c = self.cnt.clone() ;
        let x = body(self) ;
        let d = self.cnt.clone() - c ;
        (x, d)
    }

    fn put<T:Eq> (self:&mut Engine, x:T) -> Art<T,Self::Loc> { Art::Rc(Rc::new(x)) }

    fn cell<T:Eq+Debug+Clone
        +'static // TODO-Later: Needed on T because of lifetime issues.
        >
        (self:&mut Engine, nm:Self::Name, val:T) -> MutArt<T,Self::Loc> {
            let path = self.stack[0].path.clone();
            let id   = Rc::new(ArtId::Nominal(nm));
            let hash = my_hash(&(&path,&id));
            let loc  = Rc::new(Loc{path:path,id:id,hash:hash});
            info!("{} alloc cell: {:?} <--- {:?}", engineMsg!(self), &loc, &val);
            let cell = match self.table.get_mut(&loc) {
                None => None,
                Some(ref mut _nd) => {
                    Some(MutArt{loc:loc.clone(),
                                phantom:PhantomData})
                },
            } ;
            match cell {
                Some(cell) => {
                    let cell_loc = cell.loc.clone();
                    do_set(self, cell, val.clone()) ;
                },
                None => {
                    let node = Node::Mut(MutNode{
                        preds:Vec::new(),
                        val:val.clone(),
                    }) ;
                    self.table.insert(loc.clone(), Box::new(node));
                },
            } ;
            let stackLen = self.stack.len() ;
            match self.stack.last_mut() { None => (), Some(frame) => {
                let succ =
                    Succ{loc:loc.clone(),
                         dep:Rc::new(Box::new(AllocDependency{val:val})),
                         effect:Effect::Allocate,
                         dirty:false};
                info!("{} alloc cell: edge: {:?} --> {:?}", engineMsg(Some(stackLen)), &frame.loc, &loc);
                frame.succs.push(succ)
            }} ;
            MutArt{loc:loc,phantom:PhantomData}
        }

    fn set<T:Eq+Debug> (self:&mut Self, cell:MutArt<T,Self::Loc>, val:T) {
        assert!( self.stack.is_empty() ); // => outer layer has control.
        do_set(self, cell, val);
    }

    fn thunk<Arg:Eq+Hash+Debug+Clone+'static,Spurious:'static+Clone,Res:Eq+Debug+Clone+'static>
        (self:&mut Engine,
         id:ArtIdChoice<Self::Name>,
         prog_pt:ProgPt,
         fn_box:Rc<Box<Fn(&mut Engine, Arg, Spurious) -> Res>>,
         arg:Arg, spurious:Spurious)
         -> Art<Res,Self::Loc>
    {
        match id {
            ArtIdChoice::Eager => {
                Art::Rc(Rc::new(fn_box(self,arg,spurious)))
            },
            
            ArtIdChoice::Structural => {
                let hash = my_hash (&(&prog_pt, &arg)) ;
                let loc = loc_of_id(self.stack[0].path.clone(),
                                    Rc::new(ArtId::Structural(hash)));
                if false {
                    info!("{} alloc thunk: Structural {:?}\n{} ;; {:?}\n{} ;; {:?}",
                             engineMsg!(self), &loc,
                             engineMsg!(self), &prog_pt.symbol,
                             engineMsg!(self), &arg);
                } ;
                {   // If the node exists, return early.
                    let node = self.table.get_mut(&loc);
                    match node { None    => { },
                                 Some(_) => { return Art::Loc(loc) }, // Nothing to do; it already exists.
                    }
                } ;
                // assert: node does not exist.
                match self.stack.last_mut() {
                    None => (),
                    Some(frame) => {
                        let pred = frame.loc.clone();
                        let succ =
                            Succ{loc:loc.clone(),
                                 dep:Rc::new(Box::new(NoDependency)),
                                 effect:Effect::Allocate,
                                 dirty:false};
                        frame.succs.push(succ)
                    }};
                let producer : Box<Producer<Res>> =
                    Box::new(App{prog_pt:prog_pt,
                                 fn_box:fn_box,
                                 arg:arg.clone(),
                                 spurious:spurious.clone()})
                    ;
                let node : CompNode<Res> = CompNode{
                    preds:Vec::new(),
                    succs:Vec::new(),
                    producer:producer,
                    res:None,
                } ;
                self.table.insert(loc.clone(),
                                  Box::new(Node::Comp(node)));
                Art::Loc(loc)
            },
            
            ArtIdChoice::Nominal(nm) => {
                let loc = loc_of_id(self.stack[0].path.clone(),
                                    Rc::new(ArtId::Nominal(nm)));
                info!("{} alloc thunk: Nominal {:?}\n{} ;; {:?}\n{} ;; {:?}",
                         engineMsg!(self), &loc,
                         engineMsg!(self), &prog_pt.symbol,
                         engineMsg!(self), &arg);
                let producer : App<Arg,Spurious,Res> =
                    App{prog_pt:prog_pt,
                        fn_box:fn_box,
                        arg:arg.clone(),
                        spurious:spurious.clone(),
                    }
                ;
                let stackLen = self.stack.len() ;
                let (do_dirty, do_insert) = { match self.table.get_mut( &loc ) {
                    None => {
                        // do_dirty=false; do_insert=true
                        (false, true)
                    },
                    Some(node) => {
                        let node: &mut Box<GraphNode> = node ;
                        let res_nd: &mut Box<Node<Res>> = unsafe { transmute::<_,_>( node ) } ;
                        let comp_nd: &mut CompNode<Res> = match ** res_nd {                            
                            Node::Pure(_)=> unreachable!(),
                            Node::Mut(_) => panic!("TODO-Sometime"),
                            Node::Comp(ref mut comp) => comp,
                            _ => unreachable!(),
                        } ;
                        let equal_producer_prog_pts : bool =
                            comp_nd.producer.prog_pt().eq( producer.prog_pt() ) ;
                        info!("{} alloc thunk: Nominal match: equal_producer_prog_pts: {:?}",
                                 engineMsg(Some(stackLen)), equal_producer_prog_pts);
                        if equal_producer_prog_pts { // => safe cast to Box<Consumer<Arg>>
                            let app: &mut Box<App<Arg,Spurious,Res>> =
                                unsafe { transmute::<_,_>( &mut comp_nd.producer ) }
                            ;
                            info!("{} alloc thunk: Nominal match: app: {:?}", engineMsg(Some(stackLen)), app);
                            if app.get_arg() == arg {
                                // Case: Same argument; Nothing else to do:
                                // do_dirty=false; do_insert=false
                                (false, false)
                            }
                            else { // Case: Not the same argument:
                                app.consume(arg.clone()); // overwrite the old argument
                                comp_nd.res = None ; // clear the cache
                                // do_dirty=true; do_insert=false
                                (true, false)
                            }}
                        else {
                            panic!("TODO-Sometime: producers not equal!")
                        }
                    }
                } } ;
                if do_dirty {
                    info!("{} alloc thunk: dirty_alloc {:?}.", engineMsg!(self), &loc);
                    dirty_alloc(self, &loc)
                } else {
                    info!("{} alloc thunk: No dirtying.", engineMsg!(self))
                } ;
                match self.stack.last_mut() { None => (), Some(frame) => {
                    let pred = frame.loc.clone();
                    info!("{} alloc thunk: edge {:?} --> {:?}", engineMsg(Some(stackLen)), &pred, &loc);
                    let succ =
                        Succ{loc:loc.clone(),
                             dep:Rc::new(Box::new(AllocDependency{val:arg.clone()})),
                             effect:Effect::Allocate,
                             dirty:false};
                    frame.succs.push(succ)
                }};
                if do_insert {
                    let node : CompNode<Res> = CompNode{
                        preds:Vec::new(),
                        succs:Vec::new(),
                        producer:Box::new(producer),
                        res:None,
                    } ;
                    self.table.insert(loc.clone(),
                                      Box::new(Node::Comp(node)));
                    Art::Loc(loc)
                }
                else {
                    Art::Loc(loc)
                }
            }
        }
    }

    fn force<T:'static+Eq+Debug+Clone> (self:&mut Engine,
                                        art:&Art<T,Self::Loc>) -> T
    {
        match *art {
            Art::Rc(ref v) => (**v).clone(),
            Art::Loc(ref loc) => {
                let (is_comp, cached_result) : (bool, Option<T>) = {
                    let node : &mut Node<T> = res_node_of_loc(self, &loc) ;
                    match *node {
                        Node::Pure(ref mut nd) => (false, Some(nd.val.clone())),
                        Node::Mut(ref mut nd)  => (false, Some(nd.val.clone())),
                        Node::Comp(ref mut nd) => (true,  nd.res.clone()),
                        _ => panic!("undefined")
                    }
                } ;
                let result = match cached_result {
                    None => {
                        info!("{} force {:?}: cache empty", engineMsg!(self), &loc);
                        assert!(is_comp);
                        produce(self, &loc)
                    },
                    Some(ref res) => {
                        if is_comp {
                            info!("{} force {:?}: cache holds {:?}.  Using change propagation.", engineMsg!(self), &loc, &res);
                            // ProducerDep change-propagation precondition:
                            // loc is a computational node:
                            let res = ProducerDep{res:res.clone()}.change_prop(self, &loc) ;
                            info!("{} force {:?}: result changed?: {}", engineMsg!(self), &loc, res.changed) ;
                            let node : &mut Node<T> = res_node_of_loc(self, &loc) ;
                            match *node {
                                Node::Comp(ref nd) => match nd.res {
                                    None => unreachable!(),
                                    Some(ref res) =>
                                        // Testing: Reached by `pure_caching` tests
                                        res.clone()
                                },
                                _ => unreachable!(),
                            }}
                        else {
                            info!("{} force {:?}: no change prop necessary.", engineMsg!(self), &loc);
                            res.clone()
                        }
                    }
                } ;
                match self.stack.last_mut() { None => (), Some(frame) => {
                    let succ =
                        Succ{loc:loc.clone(),
                             dep:Rc::new(Box::new(ProducerDep{res:result.clone()})),
                             effect:Effect::Observe,
                             dirty:false};
                    frame.succs.push(succ);
                }} ;
                result
            }
        }}
}

pub fn main () { }

