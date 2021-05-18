
use std::io::{self,Write};
use std::collections::{BTreeMap};
use std::convert::TryInto;

use crate::config_parser::{ConfigurationValue,Expr};

///Given a list of vectors, `[A1,A2,A3,A4,...]`, `Ai` beging a `Vec<T>` and second vector `b:&Vec<T>=[b1,b2,b3,b4,...]`, each `bi:T`.
///It creates a list of vectors with each combination Ai+bj.
fn vec_product<T:Clone>(a:&Vec<Vec<T>>,b:&Vec<T>) -> Vec<Vec<T>>
{
	let mut r=vec![];
	for ae in a.iter()
	{
		for be in b.iter()
		{
			let mut new=ae.clone();
			new.push(be.clone());
			r.push(new);
		}
	}
	r
}

///Expands all the inner ConfigurationValue::Experiments given out a single ConfigurationValue::Experiments
///whose elements are free of them.
pub fn flatten_configuration_value(value:&ConfigurationValue) -> ConfigurationValue
{
	let mut names = BTreeMap::new();//name -> range
	let experiments = flatten_configuration_value_gather_names(value, &mut names);
	//println!("got names {:?}",names);
	expand_named_experiments_range(experiments,&names)
}


fn flatten_configuration_value_gather_names(value:&ConfigurationValue, names:&mut BTreeMap<String,usize>) -> ConfigurationValue
{
	match value
	{
		&ConfigurationValue::Object(ref name, ref list) =>
		{
			let mut r=vec![ vec![] ];
			for &(ref name, ref v) in list
			{
				let fv=flatten_configuration_value_gather_names(v,names);
				if let ConfigurationValue::Experiments(vlist) = fv
				{
					let factor=vlist.iter().map(|x|(name.clone(),x.clone())).collect::<Vec<(String,ConfigurationValue)>>();
					r=vec_product(&r,&factor);
				}
				else
				{
					for x in r.iter_mut()
					{
						x.push((name.clone(),fv.clone()));
					}
				}
			}
			ConfigurationValue::Experiments(r.iter().map(|values|ConfigurationValue::Object(name.clone(),values.clone())).collect())
		},
		&ConfigurationValue::Array(ref list) =>
		{
			let mut r=vec![ vec![] ];
			for ref v in list
			{
				let fv=flatten_configuration_value_gather_names(v,names);
				if let ConfigurationValue::Experiments(vlist) = fv
				{
					//let factor=vlist.iter().map(|x|x.clone()).collect::<Vec<ConfigurationValue>>();
					//r=vec_product(&r,&factor);
					r=vec_product(&r,&vlist);
				}
				else
				{
					for x in r.iter_mut()
					{
						x.push(fv.clone());
					}
				}
			}
			ConfigurationValue::Experiments(r.iter().map(|values|ConfigurationValue::Array(values.clone())).collect())
		},
		&ConfigurationValue::Experiments(ref experiments) =>
		{
			let mut r:Vec<ConfigurationValue>=vec![];
			for experiment in experiments
			{
				let flat=flatten_configuration_value_gather_names(experiment,names);
				if let ConfigurationValue::Experiments(ref flist) = flat
				{
					r.extend(flist.iter().map(|x|x.clone()));
				}
				else
				{
					r.push(flat);
				}
			}
			ConfigurationValue::Experiments(r)
		},
		&ConfigurationValue::NamedExperiments(ref name, ref experiments) =>
		{
			if let Some(&size) = names.get(name)
			{
				if size != experiments.len()
				{
					panic!("{}! has different lengths {} vs {}",name,size,experiments.len());
				}
			}
			else
			{
				names.insert(name.to_string(),experiments.len());
			}
			value.clone()
		},
		&ConfigurationValue::Where(ref v, ref _expr) =>
		{
			flatten_configuration_value_gather_names(v,names)//FIXME, filterby expr
		},
		_ => value.clone(),
	}
}

fn expand_named_experiments_range(experiments:ConfigurationValue, names:&BTreeMap<String,usize>) -> ConfigurationValue
{
	let mut r = experiments;
	for name in names.keys()
	{
		let size=*names.get(name).unwrap();
		//r=ConfigurationValue::Experiments((0..size).map(|index|{
		//	let mut context = BTreeMap::new();
		//	context.insert(key,index);
		//	match particularize_named_experiments_selected(experiments,&context)
		//	{
		//		ConfigurationValue::Experiments(ref exps) => exps,
		//		x => &vec![x],
		//	}.iter()
		//}).flatten().collect());
		let collected : Vec<Vec<ConfigurationValue>>= (0..size).map(|index|{
			let mut context : BTreeMap<String,usize> = BTreeMap::new();
			context.insert(name.to_string(),index);
			match particularize_named_experiments_selected(&r,&context)
			{
				ConfigurationValue::Experiments(exps) => exps,
				x => vec![x],
			}
		}).collect();
		r=ConfigurationValue::Experiments(collected.into_iter().map(|t|t.into_iter()).flatten().collect());
	}
	r
}

fn particularize_named_experiments_selected(value:&ConfigurationValue, names:&BTreeMap<String,usize>) -> ConfigurationValue
{
	match value
	{
		&ConfigurationValue::Object(ref name, ref list) =>
		{
			let plist = list.iter().map(|(key,x)|(key.to_string(),particularize_named_experiments_selected(x,names))).collect();
			ConfigurationValue::Object(name.to_string(),plist)
		},
		&ConfigurationValue::Array(ref list) =>
		{
			let plist = list.iter().map(|x|particularize_named_experiments_selected(x,names)).collect();
			ConfigurationValue::Array(plist)
		},
		&ConfigurationValue::Experiments(ref list) =>
		{
			let plist = list.iter().map(|x|particularize_named_experiments_selected(x,names)).collect();
			ConfigurationValue::Experiments(plist)
		},
		&ConfigurationValue::NamedExperiments(ref name, ref list) =>
		{
			if let Some(&index) = names.get(name)
			{
				list[index].clone()
			}
			else
			{
				value.clone()
			}
		},
		//&ConfigurationValue::Where(ref v, ref _expr) =>
		//{
		//	flatten_configuration_value_gather_names(v,names)//FIXME, filterby expr
		//},
		_ => value.clone(),
	}
}


///Just returns a `Context{configuration:<configuration>, result:<result>}`.
pub fn combine(configuration:&ConfigurationValue, result:&ConfigurationValue) -> ConfigurationValue
{
	ConfigurationValue::Object(String::from("Context"),vec![
		(String::from("configuration"),configuration.clone()),
		(String::from("result"),result.clone()),
	])
}

///Evaluates an expression given in a context.
///For example the expression `=Alpha.beta` will return 42 for the context `Alpha{beta:42}`.
pub fn evaluate(expr:&Expr, context:&ConfigurationValue) -> ConfigurationValue
{
	match expr
	{
		&Expr::Equality(ref a,ref b) =>
		{
			let va=evaluate(a,context);
			let vb=evaluate(b,context);
			if va==vb
			{
				ConfigurationValue::True
			}
			else
			{
				ConfigurationValue::False
			}
		},
		&Expr::Literal(ref s) => ConfigurationValue::Literal(s.clone()),
		&Expr::Number(f) => ConfigurationValue::Number(f),
		&Expr::Ident(ref s) => match context
		{
			&ConfigurationValue::Object(ref _name, ref attributes) =>
			{
				for &(ref attr_name,ref attr_value) in attributes.iter()
				{
					if attr_name==s
					{
						return attr_value.clone();
					}
				};
				panic!("There is not attribute {} in {}",s,context);
			},
			_ => panic!("Cannot evaluate identifier in non-object"),
		},
		&Expr::Member(ref expr, ref attribute) =>
		{
			let value=evaluate(expr,context);
			match value
			{
				ConfigurationValue::Object(ref _name, ref attributes) =>
				{
					for &(ref attr_name,ref attr_value) in attributes.iter()
					{
						if attr_name==attribute
						{
							return attr_value.clone();
						}
					};
					panic!("There is not member {} in {}",attribute,value);
				},
				_ => panic!("There is no member {} in {}",attribute,value),
			}
		},
		&Expr::Parentheses(ref expr) => evaluate(expr,context),
		&Expr::Name(ref expr) =>
		{
			let value=evaluate(expr,context);
			match value
			{
				ConfigurationValue::Object(ref name, ref _attributes) => ConfigurationValue::Literal(name.clone()),
				_ => panic!("{} has no name as it is not object",value),
			}
		},
		&Expr::FunctionCall(ref function_name, ref arguments) =>
		{
			match function_name.as_ref()
			{
				"lt" =>
				{
					let mut first=None;
					let mut second=None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"first" =>
							{
								first=Some(evaluate(val,context));
							},
							"second" =>
							{
								second=Some(evaluate(val,context));
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let first=first.expect("first argument of lt not given.");
					let second=second.expect("second argument of lt not given.");
					let first=match first
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("first argument of lt evaluated to a non-number ({}:?)",first),
					};
					let second=match second
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("second argument of lt evaluated to a non-number ({}:?)",second),
					};
					if first<second { ConfigurationValue::True } else { ConfigurationValue::False }
				}
				"if" =>
				{
					let mut condition=None;
					let mut true_expression=None;
					let mut false_expression=None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"condition" =>
							{
								condition=Some(evaluate(val,context));
							},
							"true_expression" =>
							{
								true_expression=Some(evaluate(val,context));
							},
							"false_expression" =>
							{
								false_expression=Some(evaluate(val,context));
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let condition=condition.expect("condition argument of if not given.");
					let true_expression=true_expression.expect("true_expression argument of if not given.");
					let false_expression=false_expression.expect("false_expression argument of if not given.");
					let condition = match condition
					{
						ConfigurationValue::True => true,
						ConfigurationValue::False => false,
						_ => panic!("if function condition did not evaluate into a Boolean value."),
					};
					if condition { true_expression } else { false_expression }
				}
				"add" =>
				{
					let mut first=None;
					let mut second=None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"first" =>
							{
								first=Some(evaluate(val,context));
							},
							"second" =>
							{
								second=Some(evaluate(val,context));
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let first=first.expect("first argument of and not given.");
					let second=second.expect("second argument of and not given.");
					let first=match first
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("first argument of add evaluated to a non-number ({}:?)",first),
					};
					let second=match second
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("second argument of add evaluated to a non-number ({}:?)",second),
					};
					ConfigurationValue::Number(first+second)
				}
				"at" =>
				{
					let mut container=None;
					let mut position=None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"container" =>
							{
								container=Some(evaluate(val,context));
							},
							"position" =>
							{
								position=Some(evaluate(val,context));
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let container=container.expect("container argument of at not given.");
					let position=position.expect("position argument of at not given.");
					let container=match container
					{
						ConfigurationValue::Array(a) => a,
						_ => panic!("first argument of at evaluated to a non-array ({}:?)",container),
					};
					let position=match position
					{
						ConfigurationValue::Number(x) => x as usize,
						_ => panic!("position argument of lt evaluated to a non-number ({}:?)",position),
					};
					container[position].clone()
				}
				"AverageBins" =>
				{
					let mut data = None;
					let mut width = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"data" =>
							{
								data=Some(evaluate(val,context));
							},
							"width" =>
							{
								width=Some(evaluate(val,context));
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let data=data.expect("data argument of at not given.");
					let width=width.expect("width argument of at not given.");
					let data=match data
					{
						ConfigurationValue::Array(a) => a,
						_ => panic!("first argument of at evaluated to a non-array ({}:?)",data),
					};
					let width=match width
					{
						ConfigurationValue::Number(x) => x as usize,
						_ => panic!("width argument of lt evaluated to a non-number ({}:?)",width),
					};
					//TODO: do we want to include incomplete bins?
					//let n = (data.len()+width-1)/width;
					let n = data.len()/width;
					//let mut result = Vec::with_capacity(n);
					let mut iter = data.into_iter();
					let result =(0..n).map(|_|{
						let mut total = 0f64;
						for _ in 0..width
						{
							total += match iter.next().unwrap()
							{
								ConfigurationValue::Number(x) => x,
								//x => panic!("AverageBins received {:?}",x),
								_ => std::f64::NAN,
							}
						}
						ConfigurationValue::Number(total/width as f64)
					}).collect();
					ConfigurationValue::Array(result)
				}
				_ => panic!("Unknown function `{}'",function_name),
			}
		}
	}
}

/// Evaluate some expressions inside a ConfigurationValue
pub fn reevaluate(value:&ConfigurationValue, context:&ConfigurationValue) -> ConfigurationValue
{
	//if let &ConfigurationValue::Expression(ref expr)=value
	//{
	//	evaluate(expr,context)
	//}
	//else
	//{
	//	value.clone()
	//}
	match value
	{
		&ConfigurationValue::Expression(ref expr) => evaluate(expr,context),
		&ConfigurationValue::Array(ref l) => ConfigurationValue::Array(l.iter().map(|e|reevaluate(e,context)).collect()),
		_ => value.clone(),
	}
}

///Get a vector of `f32` from a vector of `ConfigurationValue`s, skipping non-numeric values.
pub fn values_to_f32(list:&Vec<ConfigurationValue>) -> Vec<f32>
{
	list.iter().filter_map(|v|match v{
		&ConfigurationValue::Number(f) => Some(f as f32),
		_ => None
	}).collect()
}


///Convert a ConfigurationValue into a Vec<u8>.
///Intended to create binary files for result files.
pub fn config_to_binary(value:&ConfigurationValue) -> io::Result<Vec<u8>>
{
	//let mut map:BTreeMap<String,usize> = BTreeMap::new();
	//let mut vector:Vec<u8> = Vec::new();
	//config_into_binary(value,&mut vector,&mut map)?;
	//Ok(vector)
	let mut writer = BinaryConfigWriter::default();
	//writer.insert(value)?;
	writer.insert(value).unwrap_or_else(|e|panic!("error={:?} data={:?}",e,writer.vector));
	Ok(writer.take_vector())
}


#[derive(Debug,Default)]
pub struct BinaryConfigWriter
{
	vector:Vec<u8>,
	name_locations:BTreeMap<String,u32>
}

impl BinaryConfigWriter
{
	pub fn new() -> BinaryConfigWriter
	{
		Self::default()
	}
	pub fn take_vector(self) -> Vec<u8>
	{
		self.vector
	}
	///Append the binary version of a ConfigurationValue into a Vec<u8> using a map from names to locations inside the vector.
	///Returns the location at which it has been appended
	//pub fn config_into_binary(value:&ConfigurationValue, vector:&mut Vec<u8>, name_locations:&mut BTreeMap<String,usize>) -> io::Result<usize>
	pub fn insert(&mut self, value:&ConfigurationValue) -> io::Result<u32>
	{
		//Using little endian for everything, to allow moving the binary files between machines.
		//This is, we use to_le_bytes instead to_ne_bytes.
		let location:u32 = {
			//Align to 4 bytes
			const ALIGNMENT: usize = 4;
			let s:usize = self.vector.len();
			let r = s % ALIGNMENT;
			if r == 0 { s } else {
				let new = s + (ALIGNMENT-r);
				self.vector.resize(new, 0u8);
				new
			}.try_into().unwrap()
		};
		match value
		{
			&ConfigurationValue::Literal(ref name) => {
				self.vector.resize((location+2*4).try_into().unwrap(), 0u8);
				let loc:u32 = self.locate(name)?;
				let mut writer = &mut self.vector[location as usize..];
				writer.write_all(&0u32.to_le_bytes())?;
				writer.write_all(&loc.to_le_bytes())?;
				//match self.name_locations.get(name)
				//{
				//	Some(loc) =>{
				//		self.vector.write_all(&loc.to_le_bytes())?;
				//	},
				//	None =>{
				//		let loc = location+4;
				//		self.name_locations.insert(name.to_string(),loc);
				//		self.vector.write_all(&loc.to_ne_bytes())?;
				//		self.vector.write_all(name.as_bytes())?;
				//	},
				//};
			},
			&ConfigurationValue::Number(f) => {
				self.vector.write_all(&1u32.to_le_bytes())?;
				//using f: f64
				self.vector.write_all(&f.to_le_bytes())?;
			},
			&ConfigurationValue::Object(ref name, ref pairs) =>{
				let n:u32 = pairs.len().try_into().unwrap();
				let end = location + 8*n + 3*4;
				self.vector.resize(end as usize, 0u8);
				let loc:u32 = self.locate(name)?;
				//self.vector[location..].write_all(&2u32.to_le_bytes())?;
				let mut writer = &mut self.vector[location as usize..];
				//Write::write_all(&mut self.vector[location..],&2u32.to_le_bytes())?;
				writer.write_all(&2u32.to_le_bytes())?;
				//let mut writer = &mut self.vector[location + 4..];//this allows a drop for the string write before.
				writer.write_all(&loc.to_le_bytes())?;
				//match self.name_locations.get(name)
				//{
				//	Some(loc) =>{
				//		//self.vector[location+1*4..].write_all(&loc.to_le_bytes())?;
				//		writer.write_all(&loc.to_le_bytes())?;
				//	},
				//	None =>{
				//		let loc = end;
				//		self.name_locations.insert(name.to_string(),loc);
				//		writer.write_all(&loc.to_le_bytes())?;
				//		self.vector.write_all(name.as_bytes())?;
				//	},
				//};
				//let mut writer = &mut self.vector[location + 2*4..];//this allows a drop for the string write before.
				writer.write_all(&n.to_le_bytes())?;
				let base:usize = (location +3*4).try_into().unwrap();
				for (index,(key,val)) in pairs.iter().enumerate(){
					//write key
					let loc:u32 = self.locate(key)?;
					let mut writer = &mut self.vector[base + index*2*4 ..];
					writer.write_all(&loc.to_le_bytes())?;
					//match self.name_locations.get(key)
					//{
					//	Some(loc) =>{
					//		let mut writer = &mut self.vector[base + index*2*4 ..];
					//		writer.write_all(&loc.to_le_bytes())?;
					//	},
					//	None =>{
					//		let loc = self.vector.len();
					//		self.name_locations.insert(key.to_string(),loc);
					//		let mut writer = &mut self.vector[base + index*2*4 ..];
					//		writer.write_all(&loc.to_le_bytes())?;
					//		self.vector.write_all(key.as_bytes())?;
					//	},
					//};
					//write value
					//let loc = config_into_binary(val,self.vector,name_locations)?;
					let loc:u32 = self.insert(val)?;
					let mut writer = &mut self.vector[base + index*2*4 +4 ..];
					writer.write_all(&loc.to_le_bytes())?;
				}
			},
			&ConfigurationValue::Array(ref a) => {
				let n:u32 = a.len().try_into().unwrap();
				let end = location + 4*n + 2*4;
				self.vector.resize(end as usize, 0u8);
				let mut writer = &mut self.vector[location as usize..];
				writer.write_all(&3u32.to_le_bytes())?;
				writer.write_all(&n.to_le_bytes())?;
				let base:usize = (location +2*4).try_into().unwrap();
				for (index,val) in a.iter().enumerate(){
					let loc = self.insert(val)?;
					let mut writer = &mut self.vector[base + index*4 ..];
					writer.write_all(&loc.to_le_bytes())?;
				}
			},
			&ConfigurationValue::Experiments(ref list) => {
				let n:u32 = list.len().try_into().unwrap();
				let end = location + 4*n + 2*4;
				self.vector.resize(end as usize, 0u8);
				let mut writer = &mut self.vector[location as usize..];
				writer.write_all(&4u32.to_le_bytes())?;
				writer.write_all(&n.to_le_bytes())?;
				let base:usize = (location +2*4).try_into().unwrap();
				for (index,val) in list.iter().enumerate(){
					let loc = self.insert(val)?;
					let mut writer = &mut self.vector[base  + index*4 ..];
					writer.write_all(&loc.to_le_bytes())?;
				}
			},
			&ConfigurationValue::NamedExperiments(ref name, ref list) => {
				let n:u32 = list.len().try_into().unwrap();
				let end = location + 4*n + 3*4;
				self.vector.resize(end as usize, 0u8);
				let loc = self.locate(name)?;
				let mut writer = &mut self.vector[location as usize ..];
				writer.write_all(&5u32.to_le_bytes())?;
				writer.write_all(&loc.to_le_bytes())?;
				writer.write_all(&n.to_le_bytes())?;
				let base:usize = (location +3*4).try_into().unwrap();
				for (index,val) in list.iter().enumerate(){
					let loc = self.insert(val)?;
					let mut writer = &mut self.vector[base + index*4 ..];
					writer.write_all(&loc.to_le_bytes())?;
				}
			},
			&ConfigurationValue::True => self.vector.write_all(&6u32.to_le_bytes())?,
			&ConfigurationValue::False => self.vector.write_all(&7u32.to_le_bytes())?,
			&ConfigurationValue::Where(ref _id, ref _expr) => {
				//TODO: This is not yet supported
				//its id=8 is reserved
				self.vector.write_all(&8u32.to_le_bytes())?;
			},
			&ConfigurationValue::Expression(ref _expr) => {
				//TODO: This is not yet supported
				//its id=9 is reserved
				self.vector.write_all(&9u32.to_le_bytes())?;
			},
			&ConfigurationValue::None => self.vector.write_all(&10u32.to_le_bytes())?,
		}
		Ok(location.try_into().unwrap())
	}
	///Get a location with the name given. Insert it in the map and vector if necessary.
	fn locate(&mut self, name:&str) -> io::Result<u32>
	{
		Ok(match self.name_locations.get(name)
		{
			Some(loc) =>{
				*loc
			},
			None =>{
				let loc:u32 = self.vector.len().try_into().unwrap();
				self.name_locations.insert(name.to_string(),loc);
				self.vector.write_all(&(name.len() as u32).to_le_bytes())?;
				self.vector.write_all(name.as_bytes())?;
				loc
			},
		})
	}
}

///Read the value from the input at the given offset.
pub fn config_from_binary(data:&[u8], offset:usize) -> Result<ConfigurationValue,std::string::FromUtf8Error>
{
	let magic = u32::from_le_bytes(data[offset..offset+4].try_into().unwrap());
	//println!(">>config_from_binary data.len={} offset={} magic={}",data.len(),offset,magic);
	match magic{
		0 => {
			let loc:usize = u32::from_le_bytes(data[offset+4..offset+8].try_into().unwrap()).try_into().unwrap();
			let size:usize = u32::from_le_bytes(data[loc..loc+4].try_into().unwrap()).try_into().unwrap();
			Ok(ConfigurationValue::Literal(String::from_utf8(data[loc+4..loc+4+size].to_vec())?))
		},
		1 => {
			let f= f64::from_le_bytes(data[offset+4..offset+4+8].try_into().unwrap());
			Ok(ConfigurationValue::Number(f))
		},
		2 => {
			let loc:usize = u32::from_le_bytes(data[offset+4..offset+2*4].try_into().unwrap()).try_into().unwrap();
			let n:usize = u32::from_le_bytes(data[offset+2*4..offset+3*4].try_into().unwrap()).try_into().unwrap();
			let size:usize = u32::from_le_bytes(data[loc..loc+4].try_into().unwrap()).try_into().unwrap();
			let name = String::from_utf8(data[loc+4..loc+4+size].to_vec())?;
			let mut pairs = Vec::with_capacity(n);
			for index in 0..n
			{
				let item_offset = offset+3*4 +index*2*4;
				let loc:usize = u32::from_le_bytes(data[item_offset..item_offset+4].try_into().unwrap()).try_into().unwrap();
				let size:usize = u32::from_le_bytes(data[loc..loc+4].try_into().unwrap()).try_into().unwrap();
				let key = String::from_utf8(data[loc+4..loc+4+size].to_vec())?;
				let loc:usize = u32::from_le_bytes(data[item_offset+4..item_offset+2*4].try_into().unwrap()).try_into().unwrap();
				let val = config_from_binary(data,loc)?;
				pairs.push( (key,val) );
			}
			Ok(ConfigurationValue::Object(name,pairs))
		},
		3 => {
			let n:usize = u32::from_le_bytes(data[offset+1*4..offset+2*4].try_into().unwrap()).try_into().unwrap();
			let mut a = Vec::with_capacity(n);
			for index in 0..n
			{
				let item_offset = offset+2*4 +index*4;
				let loc:usize = u32::from_le_bytes(data[item_offset..item_offset+4].try_into().unwrap()).try_into().unwrap();
				let val = config_from_binary(data,loc)?;
				a.push( val );
			}
			Ok(ConfigurationValue::Array(a))
		},
		4 => {
			let n:usize = u32::from_le_bytes(data[offset+1*4..offset+2*4].try_into().unwrap()).try_into().unwrap();
			let mut list = Vec::with_capacity(n);
			for index in 0..n
			{
				let item_offset = offset+2*4 +index*4;
				let loc:usize = u32::from_le_bytes(data[item_offset..item_offset+4].try_into().unwrap()).try_into().unwrap();
				let val = config_from_binary(data,loc)?;
				list.push( val );
			}
			Ok(ConfigurationValue::Experiments(list))
		},
		5 => {
			let loc:usize = u32::from_le_bytes(data[offset+4..offset+2*4].try_into().unwrap()).try_into().unwrap();
			let n:usize = u32::from_le_bytes(data[offset+2*4..offset+3*4].try_into().unwrap()).try_into().unwrap();
			let size:usize = u32::from_le_bytes(data[loc..loc+4].try_into().unwrap()).try_into().unwrap();
			let name = String::from_utf8(data[loc+4..loc+4+size].to_vec())?;
			let mut list = Vec::with_capacity(n);
			for index in 0..n
			{
				let item_offset = offset+3*4 +index*4;
				let loc:usize = u32::from_le_bytes(data[item_offset..item_offset+4].try_into().unwrap()).try_into().unwrap();
				let val = config_from_binary(data,loc)?;
				list.push( val );
			}
			Ok(ConfigurationValue::NamedExperiments(name,list))
		},
		6 => Ok(ConfigurationValue::True),
		7 => Ok(ConfigurationValue::False),
		8 => panic!("binary format of where clauses is not yet supported"),
		9 => panic!("binary format of expressions is not yet supported"),
		10 => Ok(ConfigurationValue::None),
		_ => panic!("Do not know what to do with magic={}",magic),
	}
}


//#[derive(Debug,Default)]
//pub struct BinaryConfigReader
//{
//}
//
//impl BinaryConfigReader
//{
//	pub fn new() -> BinaryConfigReader
//	{
//		Self::default()
//	}
//	pub fn 
//}

