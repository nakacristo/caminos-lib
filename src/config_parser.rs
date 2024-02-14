extern crate gramatica;
use std::cmp::Ordering;
use self::gramatica::{Associativity,EarleyKind,State,Parser,ParsingTablesTrait,ParsingError};
use std::rc::Rc;
use std::fmt::{Display,Formatter,Error};
use crate::Quantifiable;
#[derive(Clone,Debug,PartialEq,PartialOrd,Quantifiable)]
pub enum ConfigurationValue{
Literal(String),
Number(f64),
Object(String,Vec<(String,ConfigurationValue)>),
Array(Vec<ConfigurationValue>),
Experiments(Vec<ConfigurationValue>),
NamedExperiments(String,Vec<ConfigurationValue>),
True,
False,
Where(Rc<ConfigurationValue>,Expr),
Expression(Expr),
None,
}

impl Default for ConfigurationValue {
fn default()->ConfigurationValue{
ConfigurationValue::None}

}
impl ConfigurationValue
{
	fn write(&self, f: &mut Formatter, indent:usize) -> Result<(),Error>
	{
		let is=String::from("\t").repeat(indent);
		write!(f,"{}",is)?;
		match self
		{
			&ConfigurationValue::Literal(ref s) => write!(f,"\"{}\"",s)?,
			&ConfigurationValue::Number(v) => write!(f,"{}",v)?,
			&ConfigurationValue::Object(ref name, ref list) =>
			{
				writeln!(f,"{}\n{}{{",name,is)?;
				for &(ref attr_name,ref attr_value) in list.iter()
				{
					writeln!(f,"{}\t{}:",is,attr_name)?;
					attr_value.write(f,indent+1)?;
					writeln!(f,",")?;
				}
				writeln!(f,"{}}}",is)?;
			},
			&ConfigurationValue::Array(ref list) =>
			{
				writeln!(f,"[")?;
				for elem in list.iter()
				{
					elem.write(f,indent+1)?;
					writeln!(f,",")?;
				}
				writeln!(f,"{}]",is)?;
			},
			&ConfigurationValue::Experiments(ref list) =>
			{
				writeln!(f,"![")?;
				for elem in list.iter()
				{
					elem.write(f,indent+1)?;
					writeln!(f,",")?;
				}
				writeln!(f,"{}]",is)?;
			},
			&ConfigurationValue::NamedExperiments(ref name, ref list) =>
			{
				writeln!(f,"{name}![")?;
				for elem in list.iter()
				{
					elem.write(f,indent+1)?;
					writeln!(f,",")?;
				}
				writeln!(f,"{}]",is)?;
			},
			&ConfigurationValue::True => write!(f,"true")?,
			&ConfigurationValue::False => write!(f,"false")?,
			&ConfigurationValue::Where(ref cv, ref _expr) => write!(f,"{} where FIXME",cv)?,
			&ConfigurationValue::Expression(ref e) => write!(f,"= {}",e)?,
			&ConfigurationValue::None => write!(f,"None")?,
		};
		Ok(())
	}
}

impl Display for ConfigurationValue {
fn fmt(&self,f:&mut Formatter)->Result<(),Error>{
self.write(f,0)}

}

#[derive(Clone,Debug,PartialEq,PartialOrd,Quantifiable)]
pub enum Expr{
Equality(Rc<Expr>,Rc<Expr>),
Literal(String),
Number(f64),
Ident(String),
Member(Rc<Expr>,String),
Parentheses(Rc<Expr>),
Name(Rc<Expr>),
FunctionCall(String,Vec<(String,Expr)>),
Array(Vec<Expr>),
}

impl Default for Expr
{
	fn default() -> Expr
	{
		Expr::Number(0.0)
	}
}

impl Display for Expr
{
	fn fmt(&self, f: &mut Formatter) -> Result<(),Error>
	{
		match self
		{
			&Expr::Literal(ref s) => write!(f,"\"{}\"",s),
			&Expr::Number(ref v) => write!(f,"{}",v),
			&Expr::Ident(ref s) => write!(f,"{}",s),
			&Expr::Member(ref expr,ref s) => write!(f,"{}.{}",expr,s),
			&Expr::Name(ref expr) => write!(f,"@{}",expr),
			&Expr::FunctionCall(ref s, ref args) => write!(f,"{}{{{}}}",s,args.iter().map(|(arg_name,arg_expr)|format!("{}:{}",arg_name,arg_expr)).collect::<Vec<String>>().join(",")),
			_ => write!(f,"fix this expr <{:?}>",self),
		}
	}
}

pub fn parse(source:& str)->Result<Token,ParsingError>{
Parser::<Token,ParsingTables>::parse(source,None,1)}

pub fn parse_expression(source:&str) -> Result<Token,ParsingError>
{
	Parser::<Token,ParsingTables>::parse(source,Some(26),1)
}

#[derive(Clone,Debug,PartialEq)]
pub enum Token{DummyStart,
True,False,Where,Number(f64),LitStr(String),Ident(String),EqualEqual,LBrace,RBrace,LBracket,RBracket,LPar,RPar,Comma,Colon,Bang,At,Equal,Dot,Value(ConfigurationValue),Object(ConfigurationValue),Members(Vec<(String,ConfigurationValue)>),Pair(String,ConfigurationValue),Array(Vec<ConfigurationValue>),Elements(Vec<ConfigurationValue>),Expression(Expr),FunctionCall(Expr),Arguments(Vec<(String,Expr)>),ExprPair(String,Expr),ExprArray(Vec<Expr>),ExprElements(Vec<Expr>),}
impl Default for Token { fn default()->Self{Token::DummyStart} }
struct ParsingTables { }
impl ParsingTablesTrait<Token> for ParsingTables {
fn initial()->usize { 20 }
#[allow(unused)]
fn match_some(parser: &mut Parser<Token,Self>) -> Option<(usize,Token)> { let source=parser.cursor;
match { match parser.keyword("true",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::True)), };
match { match parser.keyword("false",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::False)), };
match { match parser.keyword("where",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::Where)), };
match { match parser.re("-?[0-9]*\\.?[0-9]+([eE][-+]?[0-9]+)?",source) { None => None, Some((size,string)) => Some((size,string.parse::<f64>().unwrap() )) } }
{ None => (), Some((size,result)) => return Some((size,Token::Number(result))), };
{ fn _match(parser:&mut Parser<Token,ParsingTables>,source:& str)->Option<(usize,String)>{
let mut ret=None;
let mut characters=source.chars();
if (characters.next()) != (Some('"')) {} else {let mut size=1;
let mut r=String::new();
loop {match characters.next() { None => break, Some('"') => {ret = { Some((size + 1,r))};
break;}, Some('\\') => {match characters.next() { None => break, Some(c) => {r.push('\\');
r.push(c);}, }
;
size += 2;}, Some(c) => {r.push(c);
size += 1;}, }
;}}
ret}

match _match(parser,source) { None=>(), Some((size,result)) => return Some((size,Token::LitStr(result))), } };
match { match parser.re("[a-zA-Z\\x80-\\xff_][a-zA-Z0-9\\x80-\\xff_]*",source) { None => None, Some((size,string)) => Some((size,string.parse::<String>().unwrap() )) } }
{ None => (), Some((size,result)) => return Some((size,Token::Ident(result))), };
match { match parser.re("==",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::EqualEqual)), };
match { match parser.re("\\{",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::LBrace)), };
match { match parser.re("\\}",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::RBrace)), };
match { match parser.re("\\[",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::LBracket)), };
match { match parser.re("\\]",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::RBracket)), };
match { match parser.re("\\(",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::LPar)), };
match { match parser.re("\\)",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::RPar)), };
match { match parser.re(",",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::Comma)), };
match { match parser.re(":",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::Colon)), };
match { match parser.re("!",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::Bang)), };
match { match parser.re("@",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::At)), };
match { match parser.re("=",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::Equal)), };
match { match parser.re("\\.",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::Dot)), };
match { match parser.re("\\s+|\n|//[^\n]*\n|/\\*([^*]|\\*+[^/])*\\*+/",source) { None => None, Some((size,_string)) => Some((size,())) } }
{ None => (), Some((size,_result)) => return Some((size,Token::DummyStart)), };
None }//match_some
fn predict(parser:&mut Parser<Token,Self>,index:usize,state_index:usize,token:usize) { match token {
20 => {
parser.sets[index].predict(State::new(1,20,vec![5],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(2,20,vec![4],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(3,20,vec![21],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(4,20,vec![24],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(5,20,vec![16,24],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(6,20,vec![6,16,24],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(7,20,vec![1],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(8,20,vec![2],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(9,20,vec![20,3,26],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(10,20,vec![18,26],index,EarleyKind::Predict(state_index)));
}
21 => {
parser.sets[index].predict(State::new(11,21,vec![6],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(12,21,vec![6,8,9],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(13,21,vec![6,8,22,9],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(14,21,vec![6,8,22,14,9],index,EarleyKind::Predict(state_index)));
}
22 => {
parser.sets[index].predict(State::new(15,22,vec![23],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(16,22,vec![22,14,23],index,EarleyKind::Predict(state_index)));
}
23 => {
parser.sets[index].predict(State::new(17,23,vec![6,15,20],index,EarleyKind::Predict(state_index)));
}
24 => {
parser.sets[index].predict(State::new(18,24,vec![10,11],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(19,24,vec![10,25,11],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(20,24,vec![10,25,14,11],index,EarleyKind::Predict(state_index)));
}
25 => {
parser.sets[index].predict(State::new(21,25,vec![20],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(22,25,vec![25,14,20],index,EarleyKind::Predict(state_index)));
}
26 => {
parser.sets[index].predict(State::new(23,26,vec![26,7,26],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(24,26,vec![5],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(25,26,vec![4],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(26,26,vec![6],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(27,26,vec![26,19,6],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(28,26,vec![12,26,13],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(29,26,vec![17,26],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(30,26,vec![27],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(31,26,vec![30],index,EarleyKind::Predict(state_index)));
}
27 => {
parser.sets[index].predict(State::new(32,27,vec![6,8,9],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(33,27,vec![6,8,28,9],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(34,27,vec![6,8,28,14,9],index,EarleyKind::Predict(state_index)));
}
28 => {
parser.sets[index].predict(State::new(35,28,vec![29],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(36,28,vec![28,14,29],index,EarleyKind::Predict(state_index)));
}
29 => {
parser.sets[index].predict(State::new(37,29,vec![6,15,26],index,EarleyKind::Predict(state_index)));
}
30 => {
parser.sets[index].predict(State::new(38,30,vec![10,11],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(39,30,vec![10,31,11],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(40,30,vec![10,31,14,11],index,EarleyKind::Predict(state_index)));
}
31 => {
parser.sets[index].predict(State::new(41,31,vec![26],index,EarleyKind::Predict(state_index)));
parser.sets[index].predict(State::new(42,31,vec![31,14,26],index,EarleyKind::Predict(state_index)));
}
_ => panic!(""), } }//predict
#[allow(unused)]
fn compute_value(state:&mut State<Token>) { state.computed_value = Some( match state.rule { 0 => state.values[0].clone(),
1 => match &mut state.values[0] {
Token::LitStr(ref s) => Token::Value(ConfigurationValue::Literal(s.clone())),
_ => panic!(""), },
2 => match &mut state.values[0] {
Token::Number(ref v) => Token::Value(ConfigurationValue::Number(* v)),
_ => panic!(""), },
3 => match &mut state.values[0] {
Token::Object(ref mut value) => Token::Value(std::mem::take(value)),
_ => panic!(""), },
4 => match &mut state.values[0] {
Token::Array(ref mut list) => Token::Value(ConfigurationValue::Array(std::mem::take(list))),
_ => panic!(""), },
5 => match &mut state.values[0..2] {
&mut [Token::Bang,Token::Array(ref mut list)] => Token::Value(ConfigurationValue::Experiments(std::mem::take(list))),
_ => panic!(""), },
6 => match &mut state.values[0..3] {
&mut [Token::Ident(ref name),Token::Bang,Token::Array(ref mut list)] => Token::Value(ConfigurationValue::NamedExperiments(name.clone(),std::mem::take(list))),
_ => panic!(""), },
7 => match &mut state.values[0] {
Token::True => Token::Value(ConfigurationValue::True),
_ => panic!(""), },
8 => match &mut state.values[0] {
Token::False => Token::Value(ConfigurationValue::False),
_ => panic!(""), },
9 => match &mut state.values[0..3] {
&mut [Token::Value(ref mut value),Token::Where,Token::Expression(ref expr)] => Token::Value(ConfigurationValue::Where(Rc::new(std::mem::take(value)),expr.clone())),
_ => panic!(""), },
10 => match &mut state.values[0..2] {
&mut [Token::Equal,Token::Expression(ref e)] => Token::Value(ConfigurationValue::Expression(e.clone())),
_ => panic!(""), },
11 => match &mut state.values[0] {
Token::Ident(ref name) => Token::Object(ConfigurationValue::Object(name.clone(),vec![])),
_ => panic!(""), },
12 => match &mut state.values[0..3] {
&mut [Token::Ident(ref name),Token::LBrace,Token::RBrace] => Token::Object(ConfigurationValue::Object(name.clone(),vec![])),
_ => panic!(""), },
13 => match &mut state.values[0..4] {
&mut [Token::Ident(ref name),Token::LBrace,Token::Members(ref mut list),Token::RBrace] => Token::Object(ConfigurationValue::Object(name.clone(),std::mem::take(list))),
_ => panic!(""), },
14 => match &mut state.values[0..5] {
&mut [Token::Ident(ref name),Token::LBrace,Token::Members(ref mut list),Token::Comma,Token::RBrace] => Token::Object(ConfigurationValue::Object(name.clone(),std::mem::take(list))),
_ => panic!(""), },
15 => match &mut state.values[0] {
Token::Pair(ref s,ref mut value) => Token::Members(vec![(s . clone () , std :: mem :: take (value))]),
_ => panic!(""), },
16 => match &mut state.values[0..3] {
&mut [Token::Members(ref mut list),Token::Comma,Token::Pair(ref s,ref mut value)] => Token::Members({let mut new=(std::mem::take(list));
new.push((s.clone(),std::mem::take(value))); new}),
_ => panic!(""), },
17 => match &mut state.values[0..3] {
&mut [Token::Ident(ref s),Token::Colon,Token::Value(ref mut value)] => { let (x0,x1)=(s.clone(),std::mem::take(value)); Token::Pair(x0,x1) },
_ => panic!(""), },
18 => match &mut state.values[0..2] {
&mut [Token::LBracket,Token::RBracket] => Token::Array(vec![]),
_ => panic!(""), },
19 => match &mut state.values[0..3] {
&mut [Token::LBracket,Token::Elements(ref mut list),Token::RBracket] => Token::Array(std::mem::take(list)),
_ => panic!(""), },
20 => match &mut state.values[0..4] {
&mut [Token::LBracket,Token::Elements(ref mut list),Token::Comma,Token::RBracket] => Token::Array(std::mem::take(list)),
_ => panic!(""), },
21 => match &mut state.values[0] {
Token::Value(ref mut value) => Token::Elements(vec![std :: mem :: take (value)]),
_ => panic!(""), },
22 => match &mut state.values[0..3] {
&mut [Token::Elements(ref mut list),Token::Comma,Token::Value(ref mut value)] => Token::Elements({let mut new=(std::mem::take(list));
new.push(std::mem::take(value)); new}),
_ => panic!(""), },
23 => match &mut state.values[0..3] {
&mut [Token::Expression(ref left),Token::EqualEqual,Token::Expression(ref right)] => Token::Expression(Expr::Equality(Rc::new(left.clone()),Rc::new(right.clone()))),
_ => panic!(""), },
24 => match &mut state.values[0] {
Token::LitStr(ref s) => Token::Expression(Expr::Literal(s.clone())),
_ => panic!(""), },
25 => match &mut state.values[0] {
Token::Number(ref v) => Token::Expression(Expr::Number(* v)),
_ => panic!(""), },
26 => match &mut state.values[0] {
Token::Ident(ref s) => Token::Expression(Expr::Ident(s.clone())),
_ => panic!(""), },
27 => match &mut state.values[0..3] {
&mut [Token::Expression(ref path),Token::Dot,Token::Ident(ref element)] => Token::Expression(Expr::Member(Rc::new(path.clone()),element.clone())),
_ => panic!(""), },
28 => match &mut state.values[0..3] {
&mut [Token::LPar,Token::Expression(ref expr),Token::RPar] => Token::Expression(Expr::Parentheses(Rc::new(expr.clone()))),
_ => panic!(""), },
29 => match &mut state.values[0..2] {
&mut [Token::At,Token::Expression(ref expr)] => Token::Expression(Expr::Name(Rc::new(expr.clone()))),
_ => panic!(""), },
30 => match &mut state.values[0] {
Token::FunctionCall(ref value) => Token::Expression(value.clone()),
_ => panic!(""), },
31 => match &mut state.values[0] {
Token::ExprArray(ref mut list) => Token::Expression(Expr::Array(std::mem::take(list))),
_ => panic!(""), },
32 => match &mut state.values[0..3] {
&mut [Token::Ident(ref name),Token::LBrace,Token::RBrace] => Token::FunctionCall(Expr::FunctionCall(name.clone(),vec![])),
_ => panic!(""), },
33 => match &mut state.values[0..4] {
&mut [Token::Ident(ref name),Token::LBrace,Token::Arguments(ref list),Token::RBrace] => Token::FunctionCall(Expr::FunctionCall(name.clone(),list.clone())),
_ => panic!(""), },
34 => match &mut state.values[0..5] {
&mut [Token::Ident(ref name),Token::LBrace,Token::Arguments(ref list),Token::Comma,Token::RBrace] => Token::FunctionCall(Expr::FunctionCall(name.clone(),list.clone())),
_ => panic!(""), },
35 => match &mut state.values[0] {
Token::ExprPair(ref s,ref value) => Token::Arguments(vec![(s . clone () , value . clone ())]),
_ => panic!(""), },
36 => match &mut state.values[0..3] {
&mut [Token::Arguments(ref list),Token::Comma,Token::ExprPair(ref s,ref value)] => Token::Arguments({let mut new=(list.clone());
new.push((s.clone(),value.clone())); new}),
_ => panic!(""), },
37 => match &mut state.values[0..3] {
&mut [Token::Ident(ref s),Token::Colon,Token::Expression(ref expr)] => { let (x0,x1)=(s.clone(),expr.clone()); Token::ExprPair(x0,x1) },
_ => panic!(""), },
38 => match &mut state.values[0..2] {
&mut [Token::LBracket,Token::RBracket] => Token::ExprArray(vec![]),
_ => panic!(""), },
39 => match &mut state.values[0..3] {
&mut [Token::LBracket,Token::ExprElements(ref mut list),Token::RBracket] => Token::ExprArray(std::mem::take(list)),
_ => panic!(""), },
40 => match &mut state.values[0..4] {
&mut [Token::LBracket,Token::ExprElements(ref mut list),Token::Comma,Token::RBracket] => Token::ExprArray(std::mem::take(list)),
_ => panic!(""), },
41 => match &mut state.values[0] {
Token::Expression(ref mut expr) => Token::ExprElements(vec![std :: mem :: take (expr)]),
_ => panic!(""), },
42 => match &mut state.values[0..3] {
&mut [Token::ExprElements(ref mut list),Token::Comma,Token::Expression(ref mut expr)] => Token::ExprElements({let mut new=(std::mem::take(list));
new.push(std::mem::take(expr)); new}),
_ => panic!(""), },
_ => panic!(""), }) }//compute_value
fn table_terminal(token_index:usize)->bool { match token_index {
1|2|3|4|5|6|7|8|9|10|11|12|13|14|15|16|17|18|19 => true,
0|20|21|22|23|24|25|26|27|28|29|30|31 => false,
_ => panic!("table_terminal"), } }//table_terminal
fn table_priority(a:usize, b:usize) -> Option<Ordering> { match (a,b) {
(23,23) => Some(Ordering::Equal),
(23,27) => Some(Ordering::Greater),
(23,29) => Some(Ordering::Greater),
(27,23) => Some(Ordering::Less),
(27,27) => Some(Ordering::Equal),
(27,29) => Some(Ordering::Less),
(29,23) => Some(Ordering::Less),
(29,27) => Some(Ordering::Greater),
(29,29) => Some(Ordering::Equal),
_ => None, } }//table_priority
fn table_associativity(rule:usize) -> Option<Associativity> { match rule {
_ => None, } }//table_associativity
fn to_usize(token:&Token) -> usize { match token { &Token::DummyStart => 0,
&Token::True => 1,
&Token::False => 2,
&Token::Where => 3,
&Token::Number(_) => 4,
&Token::LitStr(_) => 5,
&Token::Ident(_) => 6,
&Token::EqualEqual => 7,
&Token::LBrace => 8,
&Token::RBrace => 9,
&Token::LBracket => 10,
&Token::RBracket => 11,
&Token::LPar => 12,
&Token::RPar => 13,
&Token::Comma => 14,
&Token::Colon => 15,
&Token::Bang => 16,
&Token::At => 17,
&Token::Equal => 18,
&Token::Dot => 19,
&Token::Value(_) => 20,
&Token::Object(_) => 21,
&Token::Members(_) => 22,
&Token::Pair(_,_) => 23,
&Token::Array(_) => 24,
&Token::Elements(_) => 25,
&Token::Expression(_) => 26,
&Token::FunctionCall(_) => 27,
&Token::Arguments(_) => 28,
&Token::ExprPair(_,_) => 29,
&Token::ExprArray(_) => 30,
&Token::ExprElements(_) => 31,
} }//to_usize
}//impl
