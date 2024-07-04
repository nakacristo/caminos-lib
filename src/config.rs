
use std::io::{self,Write,Read,Seek};
use std::collections::{BTreeMap};
use std::convert::TryInto;
use std::path::Path;
use std::fs::File;
//use std::rc::Rc;

use rand::{rngs::StdRng,SeedableRng};

use crate::config_parser::{self,ConfigurationValue,Expr};
use crate::event::Time;
use crate::{error,source_location};
use crate::error::*;

///Given a list of vectors, `[A1,A2,A3,A4,...]`, `Ai` being a `Vec<T>` and second vector `b:&Vec<T>=[b1,b2,b3,b4,...]`, each `bi:T`.
///It creates a list of vectors with each combination Ai+bj.
#[allow(dead_code)]
fn vec_product<T:Clone>(a:&[Vec<T>],b:&[T]) -> Vec<Vec<T>>
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

/**
Flattening was taking a lot of time for very long arrays. This version avoids many of the clones.
**/
fn vec_product_inplace<T:Clone>(a:&mut Vec<Vec<T>>,b:&[T])
{
	if b.len() == 1 {
		for ae in a.iter_mut()
		{
			ae.push(b[0].clone());
		}
	} else if b.len() == 0 {
		a.clear();
	} else {
		let a_old = std::mem::take(a);
		for ar in a_old.into_iter()
		{
			for be in b.iter()
			{
				let mut new = ar.clone();//we could skip last
				new.push(be.clone());
				a.push(new);
			}
		}
	}
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


/**
Expand those `Experiments` but not `NamedExperiments`. Collects the names of the `NamedExperiments` into the `names` map.
Panics if some `NamedExperiments` have non-matching size.
TODO: that should be an Error, not a panic.
**/
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
					//r=vec_product(&r,&factor);
					vec_product_inplace(&mut r, &factor);
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
			for v in list
			{
				let fv=flatten_configuration_value_gather_names(v,names);
				if let ConfigurationValue::Experiments(vlist) = fv
				{
					//r=vec_product(&r,&vlist);
					vec_product_inplace(&mut r, &vlist);
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
					r.extend(flist.iter().cloned());
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
			//value.clone()
			let mut r=vec![ vec![] ];
			for v in experiments
			{
				let fv=flatten_configuration_value_gather_names(v,names);
				if let ConfigurationValue::Experiments(vlist) = fv
				{
					//r=vec_product(&r,&vlist);
					vec_product_inplace(&mut r, &vlist);
				}
				else
				{
					for x in r.iter_mut()
					{
						x.push(fv.clone());
					}
				}
			}
			ConfigurationValue::Experiments(r.iter().map(|values|ConfigurationValue::NamedExperiments(name.to_string(),values.clone())).collect())
		},
		&ConfigurationValue::Where(ref v, ref _expr) =>
		{
			flatten_configuration_value_gather_names(v,names)//FIXME, filterby expr
		},
		_ => value.clone(),
	}
}

/**
Expand the `NamedExperiments`. `names[experiment_name]` is the number of entries in that `NamedExperiment`.
**/
fn expand_named_experiments_range(experiments:ConfigurationValue, names:&BTreeMap<String,usize>) -> ConfigurationValue
{
	let mut r = experiments;
	//dbg!(names);
	for name in names.keys()
	{
		//println!("name={name} current={current}",current=r.format_terminal());
		let size=*names.get(name).unwrap();
		let partials : Vec<Vec<_>> = (0..size).map(|index|{
			let mut context : BTreeMap<String,usize> = BTreeMap::new();
			context.insert(name.to_string(),index);
			match particularize_named_experiments_selected(&r,&context)
			{
				ConfigurationValue::Experiments(exps) => exps,
				x => vec![x],
			}
		}).collect();
		let count = (0..size).filter(|&index|partials[index].len()==1 && partials[index][0]==r).count();
		if count==0 {
			r=ConfigurationValue::Experiments(partials.into_iter().flat_map(|t|t.into_iter()).collect());
		} else if count == size {
			//All are equal, there is no such NamedExperiment at this branch.
			//Just keep going
		} else {
			panic!("Error when expanding {} at name {name}",r.format_terminal());
		}
	}
	r
}

/**
Expands in `value` all the `NamedExperiments` with a name in `names` to its value at index `names[name]`.
**/
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
				//list[index].clone()
				particularize_named_experiments_selected(&list[index],names)
			}
			else
			{
				//value.clone()
				let plist = list.iter().map(|x|particularize_named_experiments_selected(x,names)).collect();
				ConfigurationValue::NamedExperiments(name.to_string(),plist)
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
/// TODO: does something still uses this. Or has all being moved to `OutputEnvironmentEntry::config`.
pub fn combine(experiment_index:usize, configuration:&ConfigurationValue, result:&ConfigurationValue) -> ConfigurationValue
{
	ConfigurationValue::Object(String::from("Context"),vec![
		(String::from("index"),ConfigurationValue::Number(experiment_index as f64)),
		(String::from("configuration"),configuration.clone()),
		(String::from("result"),result.clone()),
	])
}

/**Evaluates an expression given in a context.

For example the expression `=Alpha.beta` will return 42 for the context `Alpha{beta:42}`.

# Available functions

## Comparisons

Arguments `first` and `second`. It evaluates to `ConfigurationValue::{True,False}`.

* eq or equal
* lt

## Arithmetic

* add
* mul

Arguments `first` and `second`. It evaluates to `ConfigurationValue::Number`.

## if

Evaluates to whether its argument `true_expression` or `false_expression` depending on its `condition` argument.

## at

Evaluates to the element at `position` inside the array in `container`.

## AverageBins

Evaluates to an array smaller than the input `data`, as each `width` entries are averaged into a single one.

## FileExpression

Evaluates an `expression` including the file `filename` into the current context under the name `file_data`. For example `FileExpression{filename:"peak.cfg",expression:at{container:file_data,position:index}}.maximum_value` to access into the `peak.cfg` file and evaluating the expression `at{container:file_data,position:index}`. This example assume that `peak.cfg` read into `file_data` is an array and can be accessed by `index`, the entry of the associated execution. The value returned by `FileExpression` is then accessed to its `maximum_value` field.


**/
pub fn evaluate(expr:&Expr, context:&ConfigurationValue, path:&Path) -> Result<ConfigurationValue,Error>
//pub fn evaluate<'a,C:'a,V:Into<BorrowedConfigurationValue<'a,C>>>(expr:&Expr, context:V, path:&Path) -> ConfigurationValue
//	where &'a C:Into<ConfigurationValue>, C:Clone
{
	//let context : BorrowedConfigurationValue<C> = context.into();
	match expr
	{
		&Expr::Equality(ref a,ref b) =>
		{
			let va=evaluate(a,context,path)?;
			let vb=evaluate(b,context,path)?;
			if va==vb
			{
				Ok(ConfigurationValue::True)
			}
			else
			{
				Ok(ConfigurationValue::False)
			}
		},
		&Expr::Literal(ref s) => Ok(ConfigurationValue::Literal(s.clone())),
		&Expr::Number(f) => Ok(ConfigurationValue::Number(f)),
		&Expr::Ident(ref s) => match context
		{
			ConfigurationValue::Object(ref _name, ref attributes) =>
			{
				for &(ref attr_name,ref attr_value) in attributes.iter()
				{
					if attr_name==s
					{
						return Ok(attr_value.clone());
					}
				};
				//panic!("There is not attribute {} in {}",s,context);
				return Err(error!(bad_argument).with_message(format!("There is not attribute {} in {}",s,context)));
			},
			_ => panic!("Cannot evaluate identifier in non-object"),
		},
		&Expr::Member(ref expr, ref attribute) =>
		{
			let value=evaluate(expr,context,path)?;
			match value
			{
				ConfigurationValue::Object(ref _name, ref attributes) =>
				{
					for &(ref attr_name,ref attr_value) in attributes.iter()
					{
						if attr_name==attribute
						{
							return Ok(attr_value.clone());
						}
					};
					//panic!("There is not member {} in {}",attribute,value);
					//let value = &value.to_string()[0..3000];
					let names = attributes.iter().map(|&(ref attr_name,_)|format!("{attr_name}:...")).collect::<Vec<String>>().join(", ");
					let value = format!("{{{}}}",names);
					return Err(error!(bad_argument).with_message(format!("There is no member {attribute} in {value}")));
				},
				//_ => panic!("There is no member {} in {}",attribute,value),
				_=> return Err(error!(bad_argument).with_message(format!("{value} is not an object, so it does not have member {attribute}"))),
			}
		},
		&Expr::Parentheses(ref expr) => evaluate(expr,context,path),
		&Expr::Name(ref expr) =>
		{
			let value=evaluate(expr,context,path)?;
			match value
			{
				ConfigurationValue::Object(ref name, ref _attributes) => Ok(ConfigurationValue::Literal(name.clone())),
				_ => panic!("{} has no name as it is not object",value),
			}
		},
		&Expr::FunctionCall(ref function_name, ref arguments) =>
		{
			match function_name.as_ref()
			{
				"eq" | "equal" =>
				{
					let mut first=None;
					let mut second=None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"first" =>
							{
								first=Some(evaluate(val,context,path)?);
							},
							"second" =>
							{
								second=Some(evaluate(val,context,path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let first=first.expect("first argument of lt not given.");
					let second=second.expect("second argument of lt not given.");
					//allow any type
					Ok(if first==second { ConfigurationValue::True } else { ConfigurationValue::False })
				}
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
								first=Some(evaluate(val,context,path)?);
							},
							"second" =>
							{
								second=Some(evaluate(val,context,path)?);
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
					Ok(if first<second { ConfigurationValue::True } else { ConfigurationValue::False })
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
								condition=Some(evaluate(val,context,path)?);
							},
							"true_expression" =>
							{
								true_expression=Some(evaluate(val,context,path)?);
							},
							"false_expression" =>
							{
								false_expression=Some(evaluate(val,context,path)?);
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
					Ok(if condition { true_expression } else { false_expression })
				}
				"add" | "plus" | "sum" =>
				{
					let mut first=None;
					let mut second=None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"first" =>
							{
								first=Some(evaluate(val,context,path)?);
							},
							"second" =>
							{
								second=Some(evaluate(val,context,path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let first=first.expect("first argument of and not given.");
					let second=second.expect("second argument of and not given.");
					let first=match first
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("first argument of {} evaluated to a non-number ({}:?)",function_name,first),
					};
					let second=match second
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("second argument of {} evaluated to a non-number ({}:?)",function_name,second),
					};
					Ok(ConfigurationValue::Number(first+second))
				}
				"sub" | "minus" =>
				{
					let mut first=None;
					let mut second=None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"first" =>
							{
								first=Some(evaluate(val,context,path)?);
							},
							"second" =>
							{
								second=Some(evaluate(val,context,path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let first=first.expect("first argument of and not given.");
					let second=second.expect("second argument of and not given.");
					let first=match first
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("first argument of {} evaluated to a non-number ({}:?)",function_name,first),
					};
					let second=match second
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("second argument of {} evaluated to a non-number ({}:?)",function_name,second),
					};
					Ok(ConfigurationValue::Number(first-second))
				}
				"mul" =>
				{
					let mut first=None;
					let mut second=None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"first" =>
							{
								first=Some(evaluate(val,context,path)?);
							},
							"second" =>
							{
								second=Some(evaluate(val,context,path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let first=first.expect("first argument of and not given.");
					let second=second.expect("second argument of and not given.");
					let first=match first
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("first argument of {} evaluated to a non-number ({}:?)",function_name,first),
					};
					let second=match second
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("second argument of {} evaluated to a non-number ({}:?)",function_name,second),
					};
					Ok(ConfigurationValue::Number(first*second))
				}
				"div" =>
				{
					let mut first=None;
					let mut second=None;
					let mut integer_division=false;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"first" =>
							{
								first=Some(evaluate(val,context,path)?);
							},
							"second" =>
							{
								second=Some(evaluate(val,context,path)?);
							},
							"integer" =>
                            {
                                integer_division= match evaluate(val,context,path)?
								{
									ConfigurationValue::True => true,
									ConfigurationValue::False => false,
									_ => panic!("integer argument of div did not evaluate into a Boolean value."),
								};
                            },
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let first=first.expect("first argument of and not given.");
					let second=second.expect("second argument of and not given.");
					let first=match first
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("first argument of {} evaluated to a non-number ({}:?)",function_name,first),
					};
					let second=match second
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("second argument of {} evaluated to a non-number ({}:?)",function_name,second),
					};
					let mut q = first/second;
					if integer_division {
						q = q.floor();
					}
					Ok(ConfigurationValue::Number(q))
				}
				"roundto" | "round_to" =>
				{
					let mut value=None;
					let mut precision=None;
					// TODO: avoid panics as we return a Result.
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"value" => value=Some(evaluate(val,context,path)?),
							"precision" => precision=Some(evaluate(val,context,path)?),
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let value=value.expect("value argument of roundto not given.");
					let precision=precision.expect("precision argument of roundto not given.");
					let value=match value
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("value argument of roundto evaluated to a non-number ({}:?)",value),
					};
					let precision=match precision
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("precision argument of roundto evaluated to a non-number ({}:?)",precision),
					};
					Ok(ConfigurationValue::Number((value*10f64.powf(precision)).round()/10f64.powf(precision)))
				}
				"log" =>
				{
					let mut arg=None;
					let mut base=None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"arg" =>
							{
								arg=Some(evaluate(val,context,path)?);
							},
							"base" =>
							{
								base=Some(evaluate(val,context,path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let arg=arg.expect("arg argument of and not given.");
					let arg=match arg
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("arg argument of {} evaluated to a non-number ({}:?)",function_name,arg),
					};
					let base=match base
					{
						None => 1f64.exp(),
						Some(ConfigurationValue::Number(x)) => x,
						Some(other) => panic!("base argument of {} evaluated to a non-number ({}:?)",function_name,other),
					};
					Ok(ConfigurationValue::Number(arg.log(base)))
				}
				"pow" =>
				{
					let mut exponent=None;
					let mut base=None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"exponent" =>
							{
								exponent=Some(evaluate(val,context,path)?);
							},
							"base" =>
							{
								base=Some(evaluate(val,context,path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let exponent=exponent.expect("exponent argument of and not given.");
					let exponent=match exponent
					{
						ConfigurationValue::Number(x) => x,
						_ => panic!("exponent argument of {} evaluated to a non-number ({}:?)",function_name,exponent),
					};
					let base=match base
					{
						None => 1f64.exp(),
						Some(ConfigurationValue::Number(x)) => x,
						Some(other) => panic!("base argument of {} evaluated to a non-number ({}:?)",function_name,other),
					};
					Ok(ConfigurationValue::Number(base.powf(exponent)))
				}
				"at" =>
				{
					let mut container=None;
					let mut position=None;
					let mut else_value=ConfigurationValue::None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"container" => container=Some(evaluate(val,context,path)?),
							"position" => position=Some(evaluate(val,context,path)?),
							"else" => else_value = evaluate(val,context,path)?,
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let container=container.expect("container argument of at not given.");
					let position=position.expect("position argument of at not given.");
					let container=match container
					{
						ConfigurationValue::Array(a) => a,
						_ => panic!("container argument of at evaluated to a non-array ({}:?)",container),
					};
					let position=match position
					{
						ConfigurationValue::Number(x) => x as usize,
						_ => panic!("position argument of at evaluated to a non-number ({}:?)",position),
					};
					//container[position].clone()
					if position < container.len() {
						Ok(container[position].clone())
					} else {
						Ok(else_value)
					}
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
								data=Some(evaluate(val,context,path)?);
							},
							"width" =>
							{
								width=Some(evaluate(val,context,path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let data=data.expect("data argument of at not given.");
					let width=width.expect("width argument of at not given.");
					let data=match data
					{
						ConfigurationValue::Array(a) => a,
						_ => panic!("first argument of AverageBins evaluated to a non-array ({}:?)",data),
					};
					let width=match width
					{
						ConfigurationValue::Number(x) => x as usize,
						_ => panic!("width argument of AverageBins evaluated to a non-number ({}:?)",width),
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
					Ok(ConfigurationValue::Array(result))
				}
				"JainBins" =>
				{
					let mut data = None;
					let mut width = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"data" => data=Some(evaluate(val,context,path)?),
							"width" => width=Some(evaluate(val,context,path)?),
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let data=data.expect("data argument of at not given.");
					let width=width.expect("width argument of at not given.");
					let data=match data
					{
						ConfigurationValue::Array(a) => a,
						_ => panic!("first argument of AverageBins evaluated to a non-array ({}:?)",data),
					};
					let width=match width
					{
						ConfigurationValue::Number(x) => x as usize,
						_ => panic!("width argument of AverageBins evaluated to a non-number ({}:?)",width),
					};
					//TODO: do we want to include incomplete bins?

					let n = data.len()/width;
					let mut iter = data.into_iter();
					let mut total = 0f64;
					let mut total2 = 0f64;

					for _ in 0..n
					{
						let mut sum = 0f64;
						for _ in 0..width
						{
							sum += match iter.next().unwrap()
							{
								ConfigurationValue::Number(x) => x,
								//x => panic!("AverageBins received {:?}",x),
								_ => std::f64::NAN,
							}
						}
						total += sum;
						total2 += sum*sum;
					}

					Ok(ConfigurationValue::Number(total*total/(total2*n as f64) as f64))
				}
				"FileExpression" =>
				{
					let mut filename = None;
					let mut expression = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"filename" =>
							{
								filename=Some(evaluate(val,context,path)?);
							},
							"expression" =>
							{
								expression = Some(val);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let filename=filename.expect("filename argument of at not given.");
					let expression=expression.expect("expression argument of at not given.");
					let filename = match filename
					{
						ConfigurationValue::Literal(s) => s,
						_ => panic!("filename argument of FileExpression evaluated to a non-literal ({}:?)",filename),
					};
					let file_path = path.join(filename);
					let file_data={
						let mut data = ConfigurationValue::None;
						let mut file_contents = String::new();
						let mut cfg_file=File::open(&file_path).expect("data file could not be opened");
						let mut try_raw = true;
						let mut try_binary = false;
						if try_raw
						{
							match cfg_file.read_to_string(&mut file_contents)
							{
								Ok(_) => (),
								Err(_e) => {
									//println!("Got error {} when reading",e);//too noisy
									try_raw = false;
									try_binary = true;
								}
							}
						}
						if try_raw
						{
							let parsed_file=match config_parser::parse(&file_contents)
							{
								Err(x) => panic!("error parsing data file {:?}: {:?}",file_path,x),
								Ok(x) => x,
							};
							data = match parsed_file
							{
								config_parser::Token::Value(value) =>
								{
									value
								},
								_ => panic!("Not a value. Got {:?}",parsed_file),
							}
						}
						if try_binary
						{
							let mut contents = vec![];
							cfg_file.rewind().expect("some problem rewinding data file");
							cfg_file.read_to_end(&mut contents).expect("something went wrong reading binary data");
							data=config_from_binary(&contents,0).expect("something went wrong while deserializing binary data");
						}
						data
					};
					let context = match context{
						ConfigurationValue::Object(name, data) =>
						{
							let mut content = data.clone();
							content.push( (String::from("file_data"), file_data ) );
							ConfigurationValue::Object(name.to_string(),content)
						},
						_ => panic!("wrong context"),
					};
					evaluate( expression, &context, path)
				}
				"map" =>
				{
					let mut container = None;
					let mut binding = None;
					let mut expression = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"container" =>
							{
								container=Some(evaluate(val,context,path)?);
							},
							"binding" =>
							{
								binding=Some(evaluate(val,context,path)?);
							},
							"expression" =>
							{
								expression = Some(val);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let container=container.expect("container argument of at not given.");
					let expression=expression.expect("expression argument of at not given.");
					let binding=match binding
					{
						None => "x".to_string(),
						Some(ConfigurationValue::Literal(s)) => s,
						Some(other) => panic!("{:?} cannot be used as binding variable",other),
					};
					let container=match container
					{
						ConfigurationValue::Array(a) => a,
						ConfigurationValue::None => return Ok(ConfigurationValue::None),
						_ => panic!("first argument of at evaluated to a non-array ({}:?)",container),
					};
					//let container = container.into_iter().map(|item|{
					//	let context = match context{
					//		ConfigurationValue::Object(name, data) =>
					//		{
					//			let mut content = data.clone();
					//			content.push( (binding.clone(), item ) );
					//			ConfigurationValue::Object(name.to_string(),content)
					//		},
					//		_ => panic!("wrong context"),
					//	};
					//	evaluate( expression, &context, path)
					//}).collect();
					let mut context = match context{
						ConfigurationValue::Object(name, data) =>
						{
							let mut content = data.clone();
							content.push( (binding.clone(), ConfigurationValue::None ) );
							ConfigurationValue::Object(name.to_string(),content)
						},
						_ => panic!("wrong context"),
					};
					let container = container.into_iter().map(|item|{
						match &mut context{
							ConfigurationValue::Object(_name, data) =>
							{
								data.last_mut().unwrap().1 = item;
							},
							_ => panic!("wrong context"),
						};
						evaluate( expression, &context, path)
					}).collect::<Result<_,_>>()?;
					Ok(ConfigurationValue::Array(container))
				}
				"slice" =>
				{
					let mut container = None;
					let mut start = None;
					let mut end = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"container" =>
							{
								container=Some(evaluate(val,context,path)?);
							},
							"start" =>
							{
								start= match evaluate(val,context,path)?
								{
									ConfigurationValue::Number(n) => Some(n as usize),
									_ => panic!("the start argument of slice must be a number"),
								};
							},
							"end" =>
							{
								end= match evaluate(val,context,path)?
								{
									ConfigurationValue::Number(n) => Some(n as usize),
									_ => panic!("the start argument of slice must be a number"),
								};
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let container=container.expect("container argument of at not given.");
					let container=match container
					{
						ConfigurationValue::Array(a) => a,
						_ => panic!("first argument of at evaluated to a non-array ({}:?)",container),
					};
					let start=start.unwrap_or(0);
					let end=match end
					{
						None => container.len(),
						Some(n) => n.min(container.len()),
					};
					let container = container[start..end].to_vec();
					Ok(ConfigurationValue::Array(container))
				}
				// TODO: document
				"sum_group" =>
					{
						let mut container = None;
						let mut box_size = None;
						for (key,val) in arguments
						{
							match key.as_ref()
							{
								"container" =>
									{
										container=Some(evaluate(val,context,path)?);
									},
								"box_size" =>
								{
									box_size=Some(evaluate(val,context,path)?);
								},
								_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
							}
						}
						let container=container.expect("container argument of at not given.");
						let container=match container
						{
							ConfigurationValue::Array(a) => a,
							_ => panic!("first argument of at evaluated to a non-array ({}:?)",container),
						};
						let box_size=match box_size
						{
							Some(ConfigurationValue::Number(x)) => x as usize,
							_ => panic!("box_size argument of AverageBins evaluated to a non-number ({:?}:?)",box_size),
						};
						let n = if container.len() % box_size == 0 {
							container.len() / box_size
						}else {
							container.len() / box_size + 1
						};
						let mut result = Vec::with_capacity(n);
						for i in 0..n
						{
							let mut sum = 0f64;
							for j in 0..box_size
							{
								let index = i*box_size + j;
								if index < container.len()
								{
									sum += match container[index]
									{
										ConfigurationValue::Number(x) => x,
										_ => 0.0,
									}
								}
							}
							result.push(ConfigurationValue::Number(sum));
						}
						Ok(ConfigurationValue::Array(result))
					}
				"sort" =>
				{
					let mut container = None;
					let mut expression = None;
					let mut binding = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"container" =>
							{
								container=Some(evaluate(val,context,path)?);
							},
							"expression" =>
							{
								expression=Some(val);
							},
							"binding" =>
							{
								binding=Some(evaluate(val, context, path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let container=container.expect("container argument of at not given.");
					let mut container=match container
					{
						ConfigurationValue::Array(a) => a,
						_ => panic!("first argument of at evaluated to a non-array ({}:?)",container),
					};
					let expression = match expression
					{
						None => {
							// If there is no expression just sort the array by its value.
							container.sort_by(|a,b|a.partial_cmp(b).unwrap());
							return Ok(ConfigurationValue::Array(container));
						}
						Some(expr) => expr,
					};
					let mut context = context.clone();//A single whole clone.
					let binding=match binding
					{
						None => "x".to_string(),
						Some(ConfigurationValue::Literal(s)) => s,
						Some(other) => panic!("{:?} cannot be used as binding variable",other),
					};
					// When given an expression we first compute the expression for each element of the array, making tuples (expression_value,entry).
					let mut container : Vec<( ConfigurationValue, ConfigurationValue )> = container.into_iter().map(|entry|{
						// We clone the context only once. Then push and pop onto it to keep it the same between iterations.
						if let ConfigurationValue::Object(_name,ref mut data) = &mut context
						{
							data.push( (binding.clone(), entry.clone()) );
						}
						//let expr_value = evaluate(expression, &context, path).unwrap_or_else(|e|panic!("error {} in sort function",e));
						let expr_value = evaluate(expression, &context, path)?;
						if let ConfigurationValue::Object(_name,ref mut data) = &mut context
						{
							data.pop();
						}
						Ok( (expr_value,entry) )
					}).collect::<Result<_,_>>()?;
					container.sort_by(|(a,_),(b,_)|a.partial_cmp(b).unwrap());
					let container = container.into_iter().map( |(_expr_value,entry)| entry ).collect();
					Ok(ConfigurationValue::Array(container))
				}
				// TODO: document
				"fill_list" =>
				{
					let mut container = None;
					let mut expression = None;
					let mut binding = None;
					let mut step = None;
					let mut def_value = None;
					let mut to_fill = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"container" =>
							{
								container=Some(evaluate(val,context,path)?);
							},
							"expression" =>
							{
								expression=Some(val);
							},
							"binding" =>
							{
								binding=Some(evaluate(val, context, path)?);
							},
							"to_fill" =>
							{
								to_fill=Some(val);
							},
							"step" =>
							{
								step=Some(evaluate(val, context, path)?);
							},
							"default" =>
							{
								def_value=Some(evaluate(val, context, path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let container=container.expect("container argument of at not given.");
					let mut container=match container
					{
						ConfigurationValue::Array(a) => a,
						_ => panic!("first argument of at evaluated to a non-array ({}:?)",container),
					};
					let expression = match expression
					{
						None => {
							// If there is no expression just sort the array by its value.
							container.sort_by(|a,b|a.partial_cmp(b).unwrap());
							return Ok(ConfigurationValue::Array(container));
						}
						Some(expr) => expr,
					};
					let mut context = context.clone();//A single whole clone.
					let binding=match binding
					{
						None => "x".to_string(),
						Some(ConfigurationValue::Literal(s)) => s,
						Some(other) => panic!("{:?} cannot be used as binding variable",other),
					};
					let to_fill=to_fill.expect("to_fill argument of at not given.");
					let _step=match step
					{
						None => 1,
						Some(ConfigurationValue::Number(n)) => n as usize,
						Some(other) => panic!("{:?} cannot be used as step",other),
					};
					let def_value=def_value.unwrap_or(ConfigurationValue::Number(0f64));

					// When given an expression we first compute the expression for each element of the array, making tuples (expression_value,entry).
					let mut container : Vec<( ConfigurationValue, ConfigurationValue )> = container.into_iter().map(|entry|{
						// We clone the context only once. Then push and pop onto it to keep it the same between iterations.
						if let ConfigurationValue::Object(_name,ref mut data) = &mut context
						{
							data.push( (binding.clone(), entry.clone()) );
						}
						//let expr_value = evaluate(expression, &context, path).unwrap_or_else(|e|panic!("error {} in sort function",e));
						let expr_value = evaluate(to_fill, &context, path)?;
						if let ConfigurationValue::Object(_name,ref mut data) = &mut context
						{
							data.pop();
						}
						Ok( (expr_value,entry) )
					}).collect::<Result<_,_>>()?;
					container.sort_by(|(a,_),(b,_)|a.partial_cmp(b).unwrap()); //Sort till here

					//Now fill the missing expression values with the default value, or the expression value
					let mut next_to_write = 0;
					let mut result = vec![];
					for (expr_value,entry) in container
					{
						if let ConfigurationValue::Object(_name,ref mut data) = &mut context
						{
							data.push( (binding.clone(), entry.clone()) );
						}
						//let expr_value = evaluate(expression, &context, path).unwrap_or_else(|e|panic!("error {} in sort function",e));
						let actual_value = evaluate(expression, &context, path)?;
						if let ConfigurationValue::Object(_name,ref mut data) = &mut context
						{
							data.pop();
						}

						let n = match expr_value
						{
							ConfigurationValue::Number(n) => n as usize,
							_ => panic!("fill_list expression did not evaluate to a number"),
						};
						// println!("position = {}, next_to_write = {}", n, next_to_write);
						if result.len() != next_to_write
						{
							println!("result.len() = {}, next_to_write = {}", result.len(), next_to_write);
							println!("result = {:?}", result);
							panic!();
						}

						if n > next_to_write
						{
							for _ in next_to_write..n //fill the space
							{
								result.push(def_value.clone());
							}
						}
						result.push(actual_value.clone());

						// check that the result len is n+1
						 if result.len() != n+1
						 {
						 	println!("result.len() = {}, n = {}", result.len(), n);
						 	panic!();
						 }
						next_to_write = n+1;
					}
					Ok(ConfigurationValue::Array(result))
				},
				"last" =>
				{
					let mut container = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"container" =>
							{
								container=Some(evaluate(val,context,path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let container=container.unwrap_or_else(||panic!("container argument of {} not given.",function_name));
					let container=match container
					{
						ConfigurationValue::Array(a) => a,
						_ => panic!("first argument of at evaluated to a non-array ({}:?)",container),
					};
					Ok(container.last().expect("there is not last element in the array").clone())
				}
				"number_or" =>
				// Returns the argument unchanged if it is a number, otherwise return the default value.
				{
					let mut arg = None;
					let mut default = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"arg" =>
							{
								arg=Some(evaluate(val,context,path)?);
							},
							"default" =>
							{
								default=Some(evaluate(val,context,path));
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let arg=arg.expect("arg argument of number_or not given.");
					let default=default.expect("default argument of number_or not given.");
					match arg
					{
						ConfigurationValue::Number(n) => Ok(ConfigurationValue::Number(n)),
						_ => default,
					}
				}
				"filter" =>
				{
					let mut container = None;
					let mut expression = None;
					let mut binding = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"container" =>
							{
								container=Some(evaluate(val,context,path)?);
							},
							"expression" =>
							{
								expression=Some(val);
							},
							"binding" =>
							{
								binding=Some(evaluate(val, context, path)?);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let container=container.expect("container argument of filter not given.");
					let expression=expression.expect("expression argument of filter not given.");
					let binding = match binding
					{
						None => String::from("x"),
						Some(ConfigurationValue::Literal(s)) => s,
						Some(b) => panic!("binding argument of filter evaluated to a non-literal ({}:?)",b),
					};
					let container=match container
					{
						ConfigurationValue::Array(a) => a,
						_ => panic!("first argument of at evaluated to a non-array ({:?})",container),
					};
					let mut context = match context{
						ConfigurationValue::Object(name, data) =>
						{
							let mut content = data.clone();
							content.push( (binding.clone(), ConfigurationValue::None ) );
							ConfigurationValue::Object(name.to_string(),content)
						},
						_ => panic!("wrong context"),
					};
					let container = container.into_iter().filter_map(|item|
					{
						match &mut context{
							ConfigurationValue::Object(_name, data) =>
							{
								data.last_mut().unwrap().1 = item.clone();
							},
							_ => panic!("wrong context"),
						};
						let b = evaluate(expression,&context,path);
						match b
						{
							Err(_) => Some(b),
							Ok(ConfigurationValue::True) => Some(Ok(item)),
							Ok(ConfigurationValue::False) => None,
							b => Some(Err(error!(bad_argument)
								.with_message(format!("filter expression evaluated to a non-Boolean ({:?})",b))
							)),
						}
					}).collect::<Result<_,_>>()?;
					Ok(ConfigurationValue::Array(container))
				}
				"try" =>
				{
					let mut expression=None;
					let mut default = None;
					for (key,val) in arguments
					{
						match key.as_ref()
						{
							"expression" =>
							{
								//condition=Some(evaluate(val,context,path)?);
								expression = Some(val);
							},
							"default" =>
							{
								//default=Some(evaluate(val,context,path)?);
								default = Some(val);
							},
							_ => panic!("unknown argument `{}' for function `{}'",key,function_name),
						}
					}
					let expression=expression.expect("expression argument of number_or not given.");
					let value = match evaluate(expression,context,path)
					{
						Ok(value) => value,
						Err(_) => if let Some(d) = default {
								evaluate(d,context,path)?
							} else {
								ConfigurationValue::None
							},
					};
					Ok(value)
				}
				_ => panic!("Unknown function `{}'",function_name),
			}
		},
		&Expr::Array(ref list) => {
			Ok(ConfigurationValue::Array(list.iter().map(|e|evaluate(e,context,path)).collect::<Result<Vec<_>,_>>()?))
		},
	}
}

/// Evaluate some expressions inside a ConfigurationValue
pub fn reevaluate(value:&ConfigurationValue, context:&ConfigurationValue, path:&Path) -> Result<ConfigurationValue,Error>
{
	//if let &ConfigurationValue::Expression(ref expr)=value
	//{
	//	evaluate(expr,context,path)
	//}
	//else
	//{
	//	value.clone()
	//}
	match value
	{
		&ConfigurationValue::Expression(ref expr) => evaluate(expr,context,path),
		&ConfigurationValue::Array(ref l) => Ok(ConfigurationValue::Array(
			l.iter()
				.map(|e|reevaluate(e,context,path))
				.collect::<Result<_,_>>()?
		)),
		_ => Ok(value.clone()),
	}
}

///Get a vector of `f32` from a vector of `ConfigurationValue`s, skipping non-numeric values.
pub fn values_to_f32(list:&[ConfigurationValue]) -> Vec<f32>
{
	list.iter().filter_map(|v|match v{
		&ConfigurationValue::Number(f) => Some(f as f32),
		_ => None
	}).collect()
}

///Get a vector of `f32` from a vector of `ConfigurationValue`s, skipping non-numeric values.
///It also counts the number of good, `None`, and other values.
pub fn values_to_f32_with_count(list:&Vec<ConfigurationValue>) -> (Vec<f32>,usize,usize,usize)
{
	let mut values = Vec::with_capacity(list.len());
	let mut good_count=0;
	let mut none_count=0;
	let mut other_count=0;
	for v in list
	{
		match v
		{
			&ConfigurationValue::Number(f) =>
			{
				values.push(f as f32);
				good_count+=1;
			},
			&ConfigurationValue::None => none_count+=1,
			_ => other_count+=1,
		}
	}
	(values,good_count,none_count,other_count)
}


///Converts a [ConfigurationValue] into a `Vec<u8>`.
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
	///Append the binary version of a [ConfigurationValue] into a `Vec<u8>` using a map from names to locations inside the vector.
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
		Ok(location)
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
#[allow(clippy::identity_op)]
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


///Rewrites the value in-place.
///If `edition` is `term=new_value` where `term` can be interpreted as a left-value then replace its content with `new_value`.
///returns `true` is something in `value` has been changed.
pub fn rewrite_eq(value:&mut ConfigurationValue, edition:&Expr, path:&Path) -> bool
{
	match edition
	{
		Expr::Equality(left,right) =>
		{
			//let new_value = evaluate(right,value,path);
			let new_value = match evaluate(right,value,path)
			{
				Ok(x) => x,
				Err(_e) => return false,
			};
			//rewrite_pair(value,left,new_value)
			if let Some(ptr) = config_mut_into(value,left)
			{
				*ptr = new_value;
				true
			} else {
				false
			}
		}
		_ => false,
	}
}

///Rewrites the value in-place.
///If `path_expr` can be interpreted as a left-value then replace its content with `new_value`.
///returns `true` is something in `value` has been changed.
pub fn rewrite_pair(value:&mut ConfigurationValue, path_expr:&Expr, new_value:&Expr, path:&Path) -> bool
{
	let new_value = match evaluate(new_value,value,path)
	{
		Ok(x) => x,
		Err(_e) => return false,
	};
	if let Some(ptr) = config_mut_into(value,path_expr)
	{
		*ptr = new_value;
		true
	} else {
		false
	}
}

///Rewrites the value in-place.
///If `path_expr` can be interpreted as a left-value then replace its content with `new_value`.
///returns `true` is something in `value` has been changed.
pub fn rewrite_pair_value(value:&mut ConfigurationValue, path_expr:&Expr, new_value:ConfigurationValue) -> bool
{
	if let Some(ptr) = config_mut_into(value,path_expr)
	{
		*ptr = new_value;
		true
	} else {
		false
	}
}

///Tries to access to a given path inside a ConfigurationValue
///Returns `None` if the path is not found.
pub fn config_mut_into<'a>(value:&'a mut ConfigurationValue, expr_path:&Expr) -> Option<&'a mut ConfigurationValue>
{
	match expr_path
	{
		Expr::Ident(ref name) =>
		{
			match value
			{
				ConfigurationValue::Object(ref _object_name,ref mut arr) =>
				{
					for (key,val) in arr.iter_mut()
					{
						if key==name
						{
							return Some(val);
						}
					}
					None
				}
				_ => None,
			}
		}
		Expr::Member(ref parent, ref field_name) =>
		{
			match config_mut_into(value,parent)
			{
				Some(into_parent) => config_mut_into(into_parent,&Expr::Ident(field_name.clone())),
				None => None,
			}
		}
		_ =>
		{
			None
		}
	}
}

/// Less strict than PartialEq
/// Ignores the fields `legend_name`, and `launch_configurations`.
pub fn config_relaxed_cmp(a:&ConfigurationValue, b:&ConfigurationValue) -> bool
{
	use ConfigurationValue::*;
	let ignore = |key| key == "legend_name" || key == "launch_configurations";
	match (a,b)
	{
		(Literal(sa),Literal(sb)) => sa==sb,
		(Number(xa),Number(xb)) => xa==xb,
		(Object(na,xa),Object(nb,xb)) =>
		{
			//na==nb && xa==xb,
			if na != nb { return false; }
			//do we want to enforce order of the fields?
			for ( (ka,va),(kb,vb) ) in
				xa.iter().filter(|(key,_)| !ignore(key) ).zip(
				xb.iter().filter(|(key,_)| !ignore(key)  ) )
			{
				if ka != kb { return false; }
				if !config_relaxed_cmp(va,vb) { return false; }
			}
			return true;
		}
		(Array(xa),Array(xb)) =>
		{
			if xa.len() != xb.len() { return false; }
			//xa==xb
			for (va,vb) in
				xa.iter().zip(
				xb.iter() )
			{
				if !config_relaxed_cmp(va,vb) { return false; }
			}
			return true;
		}
		(Experiments(xa),Experiments(xb)) =>
		{
			if xa.len() != xb.len() { return false; }
			//xa==xb
			for (va,vb) in
				xa.iter().zip(
				xb.iter() )
			{
				if !config_relaxed_cmp(va,vb) { return false; }
			}
			return true;
		}
		(NamedExperiments(na,xa),NamedExperiments(nb,xb)) =>
		{
			//na==nb && xa==xb,
			if na != nb { return false; }
			if xa.len() != xb.len() { return false; }
			for (va,vb) in
				xa.iter().zip(
				xb.iter() )
			{
				if !config_relaxed_cmp(va,vb) { return false; }
			}
			return true;
		}
		(True,True) => true,
		(False,False) => true,
		(Where(xa,ea),Where(xb,eb)) => xa==xb && ea==eb,
		(Expression(xa),Expression(xb)) => xa==xb,
		(None,None) => true,
		_ => false,
	}
}


/// match arms against the keys of an object
/// first argument, `$cv:expr`, is the ConfigurationValue expected to be the object
/// second argument, `$name:literal`, is the name the Object should have.
/// third argument, `$valueid:ident`, is the variable name capturing the value in the object's elements
///    and can be used in the arms
/// the remaining arguments are the arms of the match.
#[macro_export]
macro_rules! match_object{
	//($cv:expr, $name:literal, $valueid:ident, $($key:literal => $arm:tt)* ) => {{
	($cv:expr, $name:literal, $valueid:ident, $($arm:tt)* ) => {{
		match_object!($cv,[$name],$valueid,$($arm)*)
	}};
	($cv:expr, $names:expr, $valueid:ident, $($arm:tt)* ) => {{
		//Error::$kind( source_location!(), $($args),* )
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs) = $cv
		{
			if !$names.iter().any(|&x|x==cv_name)
			{
				if $names.len()==1 {
					panic!("A {} must be created from a `{}` object not `{}`",$names[0],$names[0],cv_name);
				} else {
					panic!("Trying to create either of `{:?}` object from `{}`",$names,cv_name);
				}
			}
			for &(ref name,ref $valueid) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					//"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,..arg})),
					$( $arm )*
					"legend_name" => (),
					//_ => panic!("Nothing to do with field {} in {}",name,$name),
					_ => return Err(error!(ill_formed_configuration,$cv.clone()).with_message(format!("Nothing to do with field {} in {}",name,$names.get(0).unwrap_or_else(||&"None")))),
				}
			}
		}
		else
		{
			//panic!("Trying to create a {} from a non-Object",$name);
			return Err(error!(ill_formed_configuration,$cv.clone()).with_message(format!("Trying to create a {} from a non-Object",$names.get(0).unwrap_or_else(||&"None"))));
		}
	}};
}
///Like `match_object!` but panicking on errors.
#[macro_export]
macro_rules! match_object_panic{
	($cv:expr, $name:literal, $valueid:ident ) => {{
		match_object_panic!($cv,[$name],$valueid,)
	}};
	($cv:expr, $name:literal, $valueid:ident, $($arm:tt)* ) => {{
		match_object_panic!($cv,[$name],$valueid,$($arm)*)
	}};
	($cv:expr, $names:expr, $valueid:ident, $($arm:tt)* ) => {{
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs) = $cv
		{
			if !$names.iter().any(|&x|x==cv_name)
			{
				if $names.len()==1 {
					panic!("A {} must be created from a `{}` object not `{}`",$names[0],$names[0],cv_name);
				} else {
					panic!("Trying to create either of `{:?}` object from `{}`",$names,cv_name);
				}
			}
			for &(ref name,ref $valueid) in cv_pairs
			{
				match AsRef::<str>::as_ref(&name)
				{
					$( $arm )*
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in {}",name,$names[0]),
				}
			}
		}
		else
		{
			panic!("Trying to create a {} from a non-Object",$names[0]);
		}
	}};
}

impl ConfigurationValue
{
	pub fn as_bool(&self) -> Result<bool,Error>
	{
		match self
		{
			&ConfigurationValue::True => Ok(true),
			&ConfigurationValue::False => Ok(false),
			_ => Err(error!(ill_formed_configuration, self.clone() )),
		}
	}
	pub fn as_str(&self) -> Result<&str,Error>
	{
		match self
		{
			&ConfigurationValue::Literal(ref s) => Ok(s),
			_ => Err(error!(ill_formed_configuration, self.clone() )),
		}
	}
	pub fn as_f64(&self) -> Result<f64,Error>
	{
		match self
		{
			&ConfigurationValue::Number(x) => Ok(x),
			_ => Err(error!(ill_formed_configuration, self.clone() )),
		}
	}
	pub fn as_usize(&self) -> Result<usize,Error>
	{
		match self
		{
			&ConfigurationValue::Number(x) =>{
				let res =  x as usize;
				// Casting from a float to an integer will round the float towards zero
				// overflows and underflows will saturate
				// Casting from an integer to float will produce the closest possible float
				let y = res as f64;
				let tolerance = 1e-5;
				if x-y > tolerance || x-y < -tolerance {
					Err(error!(ill_formed_configuration, self.clone()))
				} else {
					Ok( res )
				}
			},
			_ => Err(error!(ill_formed_configuration, self.clone() )),
		}
	}
	pub fn as_i32(&self) -> Result<i32,Error>
	{
		match self
		{
			&ConfigurationValue::Number(x) =>{
				let res =  x as i32;
				// Casting from a float to an integer will round the float towards zero
				// overflows and underflows will saturate
				// Casting from an integer to float will produce the closest possible float
				let y = res as f64;
				let tolerance = 1e-5;
				if x-y > tolerance || x-y < -tolerance {
					Err(error!(ill_formed_configuration, self.clone()))
				} else {
					Ok( res )
				}
			},
			_ => Err(error!(ill_formed_configuration, self.clone() )),
		}
	}
	pub fn as_time(&self) -> Result<Time,Error>
	{
		match self
		{
			&ConfigurationValue::Number(x) =>{
				let res =  x as Time;
				// Casting from a float to an integer will round the float towards zero
				// overflows and underflows will saturate
				// Casting from an integer to float will produce the closest possible float
				let y = res as f64;
				let tolerance = 1e-5;
				if x-y > tolerance || x-y < -tolerance {
					Err(error!(ill_formed_configuration, self.clone()))
				} else {
					Ok( res )
				}
			},
			_ => Err(error!(ill_formed_configuration, self.clone() )),
		}
	}
	pub fn as_array(&self) -> Result<&Vec<ConfigurationValue>,Error>
	{
		match self
		{
			&ConfigurationValue::Array(ref x) => Ok(x),
			_ => Err(error!(ill_formed_configuration, self.clone() )),
		}
	}
	pub fn as_expr(&self) -> Result<&Expr,Error>
	{
		match self
		{
			&ConfigurationValue::Expression(ref e) => Ok(e),
			_ => Err(error!(ill_formed_configuration, self.clone() )),
		}
	}
	pub fn as_rng(&self) -> Result<StdRng,Error>
	{
		match self
		{
			&ConfigurationValue::Number(x) =>{
				let seed =  x as u64;
				// Casting from a float to an integer will round the float towards zero
				// overflows and underflows will saturate
				// Casting from an integer to float will produce the closest possible float
				let y = seed as f64;
				let tolerance = 1e-5;
				if x-y > tolerance || x-y < -tolerance {
					Err(error!(ill_formed_configuration, self.clone()))
				} else {
					Ok( StdRng::seed_from_u64(seed) )
				}
			},
			_ => Err(error!(ill_formed_configuration, self.clone() )),
		}
	}
	/// Build a generic IllFormedConfiguration error from this ConfigurationValue.
	pub fn ill(&self,message:&str) -> Error
	{
		error!(ill_formed_configuration,self.clone()).with_message(message.to_string())
	}
	/// Convert this value into some string without newlines or commas.
	/// If not possible just return `"error".to_string()`.
	/// XXX: we could make a `or_else` method receiving a `Fn()->String`.
	pub fn to_csv_field(&self) -> String
	{
		use ConfigurationValue::*;
		match self
		{
			Literal(s) => s.to_string(),
			Number(x) => format!("{x}"),
			Object(name,attrs) => if attrs.is_empty() { name } else { "error" }.to_string(),
			True => "true".to_string(),
			False => "false".to_string(),
			None => "None".to_string(),
			_ => "error".to_string(),
		}
	}
	//pub fn borrow(&self) -> BorrowedConfigurationValue<ConfigurationValue>
	//{
	//	BorrowedConfigurationValue::from(self)
	//}
	pub fn rename(&mut self,new_name: String)
	{
		match self
		{
			&mut ConfigurationValue::Literal ( ref mut name ) => *name = new_name,
			&mut ConfigurationValue::Object( ref mut name, _ ) => *name = new_name,
			&mut ConfigurationValue::NamedExperiments( ref mut name,_) => *name = new_name,
			_ => (),
		}
	}
	pub fn depth(&self) -> u32
	{
		use ConfigurationValue::*;
		match self
		{
			Literal(_) | Number(_) | True | False | None => 0,
			Object(ref _name, ref key_val_list) => key_val_list.into_iter().map(|(_key,value)|value.depth()+1).max().unwrap_or(0),
			Array(ref list) | Experiments(ref list) | NamedExperiments(_, ref list) => list.into_iter().map(|value|value.depth()+1).max().unwrap_or(0),
			Where(_rc, _expr) => todo!(),
			Expression(_expr) => 0, //todo!(),
		}
	}
	/**
	A formatter for terminal session.
	**/
	pub fn format_terminal(&self) -> String
	{
		self.format_terminal_nesting(0)
	}
	/**
	A formatter for terminal session.
	`nesting` is the number of indentations or levels through to the current point.
	**/
	pub fn format_terminal_nesting(&self, nesting:u32) -> String
	{
		use ConfigurationValue::*;
		match self
		{
			Object(..) | Array(..) | Experiments(..) | NamedExperiments(..) => {
				let d = self.depth();
				//let (front_separator,middle_separator,back_separator) = if d<=1 {
				//	(format!(" "),format!(", "),format!(" "))
				//} else {
				//	let tab:String = (0..(nesting+1)).map(|_|'\t').collect();
				//	(format!("\n{tab}"),format!(",\n{tab}"),format!(""))
				//};
				let available_columns : usize = (200i32 - nesting as i32*8i32).try_into().unwrap_or(0);
				/*TODO: We could use some of these
				let (x, y) = termion::terminal_size().unwrap();
				let termsize::Size {rows, cols} = termsize::get().unwrap();
				TODO: Are 8 spaces per indent level a reasonably assumption?
				*/
				for try_index in 0..=1 
				{
					let (front_separator,middle_separator,back_separator) = if try_index==0 {
						let spaces:String = (0..d).map(|_|' ').collect();
						(format!("{spaces}"),format!(",{spaces}"),format!("{spaces}"))
					} else {
						let tab:String = (0..(nesting+1)).map(|_|'\t').collect();
						(format!("\n{tab}"),format!(",\n{tab}"),format!(""))
					};
					let content = match self {
						Object(ref name, ref key_val_list) => {
							if key_val_list.is_empty() {
								format!("{name}")
							} else {
								let formatted_list = key_val_list.into_iter().map(|(key,value)|{
									let formatted_value = value.format_terminal_nesting(nesting+1);
									format!("{key}: {formatted_value}")
								}).collect::<Vec<String>>().join(&middle_separator);
								format!("{name}{{{front_separator}{formatted_list}{back_separator}}}")
							}
						},
						Array(ref list) => {
							let inner = list.into_iter().map(|value|{
								value.format_terminal_nesting(nesting+1)
							}).collect::<Vec<String>>().join(&middle_separator);
							format!("[{front_separator}{inner}{back_separator}]")
						},
						Experiments(ref list) => {
							let inner = list.into_iter().map(|value|{
								value.format_terminal_nesting(nesting+1)
							}).collect::<Vec<String>>().join(&middle_separator);
							format!("![{front_separator}{inner}{back_separator}]")
						},
						NamedExperiments(ref name, ref list) => {
							let inner = list.into_iter().map(|value|{
								value.format_terminal_nesting(nesting+1)
							}).collect::<Vec<String>>().join(&middle_separator);
							format!("{name}![{front_separator}{inner}{back_separator}]")
						},
						_ => unreachable!(),
					};
					if content.len() < available_columns || try_index == 1 {
						// Either the first try if it fits on the terminal columns
						// or on the second try otherwise.
						return content;
					}
				}
				// We should have returned before.
				unreachable!();
			}
			Literal(ref s) => format!("\"{s}\""),
			Number(x) => format!("{x}"),
			True => format!("true"),
			False => format!("false"),
			Where(_rc, _expr) => todo!(),
			Expression(expr) => format!("={expr}"),
			None => format!("None"),
		}
	}
	/**
	A formatter for terminal session.
	**/
	pub fn format_latex(&self) -> String
	{
		let inner  = self.format_latex_nesting(0);
		format!("\\sloppy\\tt {inner}")
	}
	/**
	A formatter for terminal session.
	`nesting` is the number of indentations or levels through to the current point.
	**/
	pub fn format_latex_nesting(&self, nesting:u32) -> String
	{
		let d = self.depth();
		let (front_separator,middle_separator,back_separator) = if d<=1 {
			(format!(" "),format!(", "),format!(" "))
		} else {
			let tab:String = (0..(nesting+1)).map(|_|"\\hskip 1em ").collect();
			(format!("\n\\newline\\mbox{{}}{tab}"),format!(",\n\\newline\\mbox{{}}{tab}"),format!(""))
		};
		use ConfigurationValue::*;
		use crate::output::latex_protect_text;
		match self
		{
			Literal(ref s) => format!("\"\\verb|{s}|\""),
			Number(x) => format!("{x}"),
			Object(ref name, ref key_val_list) => {
				let latex_name = latex_protect_text(name);
				if key_val_list.is_empty() {
					format!("\\textit{{{latex_name}}}")
				} else {
					let formatted_list = key_val_list.into_iter().map(|(key,value)|{
						let formatted_value = value.format_latex_nesting(nesting+1);
						format!("{{\\bf{{{key}}}}}: {formatted_value}",key=latex_protect_text(key))
					}).collect::<Vec<String>>().join(&middle_separator);
					format!("\\textit{{{latex_name}}}\\{{{front_separator}{formatted_list}{back_separator}\\}}")
				}
			},
			Array(ref list) => {
				let inner = list.into_iter().map(|value|{
					value.format_latex_nesting(nesting+1)
				}).collect::<Vec<String>>().join(&middle_separator);
				format!("[{front_separator}{inner}{back_separator}]")
			},
			Experiments(ref list) => {
				let inner = list.into_iter().map(|value|{
					value.format_latex_nesting(nesting+1)
				}).collect::<Vec<String>>().join(&middle_separator);
				format!("{{\\bf ![}}{front_separator}{inner}{back_separator}{{\\bf]}}")
			},
			NamedExperiments(ref name, ref list) => {
				let latex_name = latex_protect_text(name);
				let inner = list.into_iter().map(|value|{
					value.format_latex_nesting(nesting+1)
				}).collect::<Vec<String>>().join(&middle_separator);
				format!("{{\\bf {latex_name}![}}{front_separator}{inner}{back_separator}{{\\bf]}}")
			},
			True => format!("true"),
			False => format!("false"),
			Where(_rc, _expr) => "".to_string(),//todo!(),
			Expression(_expr) => "".to_string(),//todo!(),
			None => format!("None"),
		}
	}
}







//Perhaps the best would be to implement a trait
// trait AsConfigurationValue
// fn expand() -> ExpandedConfigurationValue<Self::Base>
// impl AsConfigurationValue for ConfigurationValue
// impl AsConfigurationValue for &ConfigurationValue
// impl AsConfigurationValue for IndirectObject{name:String,content:Vec<&str,C>} where C:AsConfigurationValue

//#[derive(Debug)]
//enum BorrowedConfigurationValue<'a,C>
//{
//Literal(&'a str),
//Number(f64),
//Object(&'a str,&'a [(String,C)]),
//Array(&'a [C]),
//Experiments(&'a [C]),
//NamedExperiments(&'a str,&'a [C]),
//True,
//False,
//Where(Rc<C>,&'a Expr),
//Expression(&'a Expr),
//None,
//}
//
//impl<'a> From<&'a ConfigurationValue> for BorrowedConfigurationValue<'a,ConfigurationValue>
//{
//	fn from(cv:&'a ConfigurationValue) -> BorrowedConfigurationValue<'a,ConfigurationValue>
//	{
//		match cv
//		{
//			ConfigurationValue::Literal(s) => BorrowedConfigurationValue::Literal(s),
//			ConfigurationValue::Number(x) => BorrowedConfigurationValue::Number(*x),
//			ConfigurationValue::Object(s,a) => BorrowedConfigurationValue::Object(s,a),
//			ConfigurationValue::Array(a) => BorrowedConfigurationValue::Array(a),
//			ConfigurationValue::Experiments(a) => BorrowedConfigurationValue::Experiments(a),
//			ConfigurationValue::NamedExperiments(s,a) => BorrowedConfigurationValue::NamedExperiments(s,a),
//			ConfigurationValue::True => BorrowedConfigurationValue::True,
//			ConfigurationValue::False => BorrowedConfigurationValue::False,
//			ConfigurationValue::Where(c,e) => BorrowedConfigurationValue::Where( Rc::clone(c),e),
//			ConfigurationValue::Expression(e) => BorrowedConfigurationValue::Expression(e),
//			ConfigurationValue::None => BorrowedConfigurationValue::None,
//		}
//	}
//}
//
//impl<'a> From<&mut'a ConfigurationValue> for BorrowedConfigurationValue<'a,ConfigurationValue>
//{
//	fn from(cv:&mut'a ConfigurationValue) -> BorrowedConfigurationValue<'a,ConfigurationValue>
//	{
//		BorrowedConfigurationValue::from(&*cv)
//	}
//}
//
//impl<'a> From<&'a ConfigurationValue> for ConfigurationValue
//{
//	fn from(cv:&'a ConfigurationValue) -> ConfigurationValue
//	{
//		cv.clone()
//	}
//}
//
//impl<'a,C> From<BorrowedConfigurationValue<'a,C>> for ConfigurationValue
//	where &'a C:Into<ConfigurationValue>
//{
//	fn from(cv:BorrowedConfigurationValue<'a,C>) -> ConfigurationValue
//	{
//		match cv
//		{
//			BorrowedConfigurationValue::Literal(s) => ConfigurationValue::Literal(s.to_string()),
//			BorrowedConfigurationValue::Number(x) => ConfigurationValue::Number(x),
//			BorrowedConfigurationValue::Object(s,a) => ConfigurationValue::Object(s.to_string(),a
//				.iter().map(|(x,y)|(x.to_string(),y.into())).collect()),
//			BorrowedConfigurationValue::Array(a) => ConfigurationValue::Array(a
//				.iter().map(|x|x.into()).collect()),
//			BorrowedConfigurationValue::Experiments(a) => ConfigurationValue::Experiments(a
//				.iter().map(|x|x.into()).collect()),
//			BorrowedConfigurationValue::NamedExperiments(s,a) => ConfigurationValue::NamedExperiments(s.to_string(),a
//				.iter().map(|x|x.into()).collect()),
//			BorrowedConfigurationValue::True => ConfigurationValue::True,
//			BorrowedConfigurationValue::False => ConfigurationValue::False,
//			BorrowedConfigurationValue::Where(c,e) => ConfigurationValue::Where( Rc::new((&*c).into()),e.clone()),
//			BorrowedConfigurationValue::Expression(e) => ConfigurationValue::Expression(e.clone()),
//			BorrowedConfigurationValue::None => ConfigurationValue::None,
//		}
//	}
//}

//impl<'a,S,C,VSC,VC> Borrow<BorrowedConfigurationValue<'a,S,C,VSC,VC>> for ConfigurationValue
//	where String:Borrow<S>, ConfigurationValue:Borrow<C>,
//		Vec<(String,ConfigurationValue)>:Borrow<VSC>, Vec<ConfigurationValue>:Borrow<VC>,
//		Self:'a
//{
//	fn borrow(&self) -> &BorrowedConfigurationValue<'a,S,C,VSC,VC>
//	{
//		match self
//		{
//			&ConfigurationValue::Literal(s) => &BorrowedConfigurationValue::Literal(s.borrow()), 
//			_=> &BorrowedConfigurationValue::None,
//		}
//	}
//}
//
//impl<'a,S,C,VSC,VC> ToOwned for BorrowedConfigurationValue<'a,S,C,VSC,VC>
//	where String:Borrow<S>, ConfigurationValue:Borrow<C>,
//		Vec<(String,ConfigurationValue)>:Borrow<VSC>, Vec<ConfigurationValue>:Borrow<VC>,
//{
//	type Owned = ConfigurationValue;
//	fn to_owned(&self) -> ConfigurationValue
//	{
//		todo!()
//	}
//}


#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn config_functions()
	{
		use std::path::PathBuf;
		let context = ConfigurationValue::Object("Context".to_string(),vec![
			("a1".to_string(),ConfigurationValue::Array(vec![ConfigurationValue::Number(10.0),ConfigurationValue::Number(15.0),ConfigurationValue::Number(7.0),ConfigurationValue::Number(2.0),ConfigurationValue::Number(14.0),]))
		]);
		let path = PathBuf::from(".");
		let v1 = Expr::Number(1.0);
		let v2 = Expr::Number(2.0);
		match evaluate(&Expr::FunctionCall("add".to_string(),vec![("first".to_string(),v1),("second".to_string(),v2)]),&context,&path)
		{
			Ok( ConfigurationValue::Number(x) ) => assert_eq!(x,3.0),
			_ => assert!(false),
		}
		match evaluate(&Expr::FunctionCall("sort".to_string(),vec![("container".to_string(),Expr::Ident("a1".to_string())),("expression".to_string(),Expr::Ident("x".to_string()))]),&context,&path)
		{
			Ok( ConfigurationValue::Array(x) ) => {
				let mut it = x.iter();
				assert_eq!(it.next().unwrap().as_usize().unwrap(),2);
				assert_eq!(it.next().unwrap().as_usize().unwrap(),7);
				assert_eq!(it.next().unwrap().as_usize().unwrap(),10);
				assert_eq!(it.next().unwrap().as_usize().unwrap(),14);
				assert_eq!(it.next().unwrap().as_usize().unwrap(),15);
				assert_eq!(it.next(),None);
			}
			_ => assert!(false),
		}
	}
	#[test]
	fn flatten_test_simple()
	{
		use ConfigurationValue::*;
		let original = Object("Alpha".to_string(),vec![("a".to_string(),
			Experiments(vec![Number(1.0),Number(2.0)]),
		)]);
		let target = Experiments(vec![
			Object("Alpha".to_string(),vec![("a".to_string(),Number(1.0))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Number(2.0))]),
		]);
		assert_eq!(flatten_configuration_value(&original),target);
	}
	#[test]
	fn flatten_test_named()
	{
		use ConfigurationValue::*;
		let original = Object("Alpha".to_string(),vec![("a".to_string(),
			NamedExperiments("name".to_string(), vec![Number(1.0),Number(2.0)]),
		)]);
		let target = Experiments(vec![
			Object("Alpha".to_string(),vec![("a".to_string(),Number(1.0))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Number(2.0))]),
		]);
		assert_eq!(flatten_configuration_value(&original),target);
	}
	#[test]
	fn flatten_test_nest_anonymous_over_anonymous()
	{
		use ConfigurationValue::*;
		/*
		Alpha{a:![1.0, ![2.0,3.0]]}
		*/
		let original = Object("Alpha".to_string(),vec![("a".to_string(),
			Experiments(vec![
				Number(1.0),
				Experiments(vec![Number(2.0),Number(3.0)]),
			]),
		)]);
		let target = Experiments(vec![
			Object("Alpha".to_string(),vec![("a".to_string(),Number(1.0))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Number(2.0))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Number(3.0))]),
		]);
		assert_eq!(flatten_configuration_value(&original),target);
	}
	//FIXME: decide the intended meaning
	//#[test]
	//fn flatten_test_nest_anonymous_over_named()
	//{
	//	use ConfigurationValue::*;
	//	/*
	//		Alpha{a:![ 1.0, name![2.0,3.0] ]}
	//		->
	//		![ Alpha{a:1.0}, Alpha{a:name![2.0,3.0]} ]
	//		->
	//		![Alpha{a:1.0}, Alpha{a:2.0}, Alpha{a:1.0} Alpha{a:3.0}]
	//		...
	//		But we may want to have
	//		![Alpha{a:1.0}, Alpha{a:2.0}, Alpha{a:3.0}]
	//	*/
	//	let original = Object("Alpha".to_string(),vec![("a".to_string(),
	//		Experiments(vec![
	//			Number(1.0),
	//			NamedExperiments("name".to_string(), vec![Number(2.0),Number(3.0)]),
	//		]),
	//	)]);
	//	let target = Experiments(vec![
	//		Object("Alpha".to_string(),vec![("a".to_string(),Number(1.0))]),
	//		Object("Alpha".to_string(),vec![("a".to_string(),Number(2.0))]),
	//		Object("Alpha".to_string(),vec![("a".to_string(),Number(3.0))]),
	//	]);
	//	assert_eq!(flatten_configuration_value(&original),target);
	//}
	//FIXME: What is the intended meaning??
	//#[test]
	//fn flatten_test_nest_named_over_named()
	//{
	//	use ConfigurationValue::*;
	//	/*
	//		Alpha{a:name1![1.0,name2![2.0,3.0]]}
	//		->
	//		![ Alpha{a:1.0}, Alpha{a:name2![2.0,3.0]} ]
	//		-> ????
	//		![ Alpha{a:1.0}, Alpha{a:2.0}, Alpha{a:3.0} ]
	//		If expand name2=2.0 we get
	//			![ Alpha{a:1.0}, Alpha{a:2.0} ]
	//		and with name2=3.0 we get
	//			![ Alpha{a:1.0}, Alpha{a:3.0} ]
	//		following -> ????
	//		![ Alpha{a:1.0}, Alpha{a:2.0}, Alpha{a:1.0}, Alpha{a:3.0} ]
	//	*/
	//	let original = Object("Alpha".to_string(),vec![("a".to_string(),
	//		NamedExperiments("name1".to_string(), vec![
	//			Number(1.0),
	//			NamedExperiments("name2".to_string(), vec![Number(2.0),Number(3.0)]),
	//		]),
	//	)]);
	//	let target = Experiments(vec![
	//		Object("Alpha".to_string(),vec![("a".to_string(),Number(1.0))]),
	//		Object("Alpha".to_string(),vec![("a".to_string(),Number(2.0))]),
	//		Object("Alpha".to_string(),vec![("a".to_string(),Number(3.0))]),
	//	]);
	//	assert_eq!(flatten_configuration_value(&original),target);
	//}
	//FIXME: what is the intended meaning.
	//#[test]
	//fn flatten_test_nest_named_over_named_reversed()
	//{
	//	use ConfigurationValue::*;
	//	/*
	//		Alpha{a:name2![1.0,name1![2.0,3.0]]}
	//		->
	//		![ Alpha{a:name2![1.0,2.0]]} , Alpha{a:name2![1.0,3.0]]} ]
	//		->
	//		![ Alpha{a:1.0}, Alpha{a:1.0}, Alpha{2.0}, Alpha{3.0} ]
	//		Here the expansion at name2=1.0 is `Alpha{a:1.0}` in both cases and perhaps could be discarded.
	//	*/
	//	let original = Object("Alpha".to_string(),vec![("a".to_string(),
	//		NamedExperiments("name2".to_string(), vec![
	//			Number(1.0),
	//			NamedExperiments("name1".to_string(), vec![Number(2.0),Number(3.0)]),
	//		]),
	//	)]);
	//	let target = Experiments(vec![
	//		Object("Alpha".to_string(),vec![("a".to_string(),Number(1.0))]),
	//		Object("Alpha".to_string(),vec![("a".to_string(),Number(2.0))]),
	//		Object("Alpha".to_string(),vec![("a".to_string(),Number(3.0))]),
	//	]);
	//	assert_eq!(flatten_configuration_value(&original),target);
	//}
	#[test]
	fn flatten_test_nestarray_anonymous_over_anonymous()
	{
		use ConfigurationValue::*;
		/*
		Alpha{a:![
			[1.0, ![2.0,3.0]],
			[4.0, ![5.0,6.0]],
		]}
		->
		![
			Alpha{a:[1.0,2.0]},
			Alpha{a:[1.0,3.0]},
			Alpha{a:[4.0,5.0]},
			Alpha{a:[4.0,6.0]},
		]
		*/
		let original = Object("Alpha".to_string(),vec![("a".to_string(),
			Experiments(vec![
				Array(vec![Number(1.0),Experiments(vec![Number(2.0),Number(3.0)])]),
				Array(vec![Number(4.0),Experiments(vec![Number(5.0),Number(6.0)])]),
			]),
		)]);
		let target = Experiments(vec![
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(1.0),Number(2.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(1.0),Number(3.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(4.0),Number(5.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(4.0),Number(6.0)]))]),
		]);
		assert_eq!(flatten_configuration_value(&original),target);
	}
	#[test]
	fn flatten_test_nestarray_anonymous_over_named()
	{
		use ConfigurationValue::*;
		/*
		Alpha{a:![
			[1.0, !name[2.0,3.0]],
			[4.0, !name[5.0,6.0]],
		]}
		->
		![
			Alpha{a:[1.0,2.0]},
			Alpha{a:[4.0,5.0]},
			Alpha{a:[1.0,3.0]},
			Alpha{a:[4.0,6.0]},
		]
		The anonymous is expanded first. Then the whole block is expanded at the first index of the named, and then at the second index.
		*/
		let original = Object("Alpha".to_string(),vec![("a".to_string(),
			Experiments(vec![
				Array(vec![Number(1.0),NamedExperiments("name".to_string(),vec![Number(2.0),Number(3.0)])]),
				Array(vec![Number(4.0),NamedExperiments("name".to_string(),vec![Number(5.0),Number(6.0)])]),
			]),
		)]);
		let target = Experiments(vec![
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(1.0),Number(2.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(4.0),Number(5.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(1.0),Number(3.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(4.0),Number(6.0)]))]),
		]);
		assert_eq!(flatten_configuration_value(&original),target);
	}
	//FIXME: What is even the intended behaviour??
	//#[test]
	//fn flatten_test_nestarray_named_over_anonymous()
	//{
	//	use ConfigurationValue::*;
	//	/*
	//	Alpha{a:!name[
	//		[1.0, ![2.0,3.0]],
	//		[4.0, ![5.0,6.0]],
	//	]}
	//	->
	//	![
	//		Alpha{a:[1.0,2.0]},
	//		Alpha{a:[1.0,3.0]},
	//		Alpha{a:[4.0,5.0]},
	//		Alpha{a:[4.0,6.0]},
	//	]
	//	The anonymous is expanded first.
	//	*/
	//	let original = Object("Alpha".to_string(),vec![("a".to_string(),
	//		NamedExperiments("name".to_string(),vec![
	//			Array(vec![Number(1.0),Experiments(vec![Number(2.0),Number(3.0)])]),
	//			Array(vec![Number(4.0),Experiments(vec![Number(5.0),Number(6.0)])]),
	//		]),
	//	)]);
	//	let target = Experiments(vec![
	//		Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(1.0),Number(2.0)]))]),
	//		Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(1.0),Number(3.0)]))]),
	//		Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(4.0),Number(5.0)]))]),
	//		Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(4.0),Number(6.0)]))]),
	//	]);
	//	assert_eq!(flatten_configuration_value(&original),target);
	//}
	#[test]
	fn flatten_test_nestarray_named_over_named()
	{
		use ConfigurationValue::*;
		/*
		Alpha{a:!name1[
			[1.0, !name2[2.0,3.0]],
			[4.0, !name2[5.0,6.0]],
		]}
		->
		![
			Alpha{a:[1.0,2.0]},
			Alpha{a:[4.0,5.0]},
			Alpha{a:[1.0,3.0]},
			Alpha{a:[4.0,6.0]},
		]
		names are processed in order.
		*/
		let original = Object("Alpha".to_string(),vec![("a".to_string(),
			NamedExperiments("name1".to_string(),vec![
				Array(vec![Number(1.0),NamedExperiments("name2".to_string(),vec![Number(2.0),Number(3.0)])]),
				Array(vec![Number(4.0),NamedExperiments("name2".to_string(),vec![Number(5.0),Number(6.0)])]),
			]),
		)]);
		let target = Experiments(vec![
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(1.0),Number(2.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(4.0),Number(5.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(1.0),Number(3.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(4.0),Number(6.0)]))]),
		]);
		assert_eq!(flatten_configuration_value(&original),target);
	}
	#[test]
	fn flatten_test_nestarray_named_over_named_backwards()
	{
		use ConfigurationValue::*;
		/*
		Alpha{a:name2![
			[1.0, name1![2.0,3.0]],
			[4.0, name1![5.0,6.0]],
		]}
		->
		![
			Alpha{a:[1.0,2.0]},
			Alpha{a:[1.0,3.0]},
			Alpha{a:[4.0,5.0]},
			Alpha{a:[4.0,6.0]},
		]
		names are processed in order. First we get ![ Alpha{a:name2![[1.0,2.0],[4.0,5.0]]},  Alpha{a:name2![[1.0,3.0],[4.0,6.0]]} ].
		Then expanding name2 at first index we get ![ Alpha{a:[1.0,2.0]},  Alpha{a:[1.0,3.0]} ].
		And at second index ![ Alpha{a:[4.0,5.0]},  Alpha{a:[4.0,6.0]} ].
		*/
		let original = Object("Alpha".to_string(),vec![("a".to_string(),
			NamedExperiments("name2".to_string(),vec![
				Array(vec![Number(1.0),NamedExperiments("name1".to_string(),vec![Number(2.0),Number(3.0)])]),
				Array(vec![Number(4.0),NamedExperiments("name1".to_string(),vec![Number(5.0),Number(6.0)])]),
			]),
		)]);
		println!("original={}",original.format_terminal());
		let target = Experiments(vec![
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(1.0),Number(2.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(1.0),Number(3.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(4.0),Number(5.0)]))]),
			Object("Alpha".to_string(),vec![("a".to_string(),Array(vec![Number(4.0),Number(6.0)]))]),
		]);
		assert_eq!(flatten_configuration_value(&original),target);
	}
}

