/*!

The Polarized routing. A routing that includes many routes of many lengths.

In some topologies packets may reach corner vertices for which Polarized routing does not have legal extension. In these topologies Polarized routing should be use in combination with other scheme. For example, even something as naive as a [SumRouting] with [Mindless] may work fine.

- Camarero, C., Martínez, C., & Beivide, R. (2021, August). Polarized routing: an efficient and versatile algorithm for large direct networks. In 2021 IEEE Symposium on High-Performance Interconnects (HOTI) (pp. 52-59). IEEE.
- Camarero, C., Martínez, C., & Beivide, R. (2022). Polarized routing for large interconnection networks. IEEE Micro, 42(2), 61-67.

*/

use std::cell::RefCell;
use std::any::Any;

use crate::routing::*;
use crate::match_object_panic;

/**
The Polarized routing algorithm.
Find polarized routes in a greedy way. Tries to minimize the weight D(s,c)-D(c,t) for current c, source s, and target t.
Equal steps are only allowed in the current 'direction' (away from source or towards target).
Polarized routes have maximum length of at most `4*diameter - 3`.

# Example
```ignore
Polarized{
	/// Include the weight as label, shifted so that the lowest weight is given the label 0. Otherwise it just put a value of 0 for all.
	include_labels: true,
	/// Restrict the routes to those that strictly improve the weight function at each step.
	/// Note that mmany/most topologies benefit from using routes that have a few edges with no change to the weight.
	/// Therefore one should expect too few routes when using this option.
	/// Strong polarized routes have maximum length of at most 2*diameter.
	/// Has a default value of false.
	strong: false,
	/// Whether to raise a panic when there are no candidates. default to true.
	/// It is to be set to false when employing in conjunction with another routing when Polarized return an empty set of routes.
	//panic_on_empty: true,
	/// Builds a `PolarizedStatistics{empty_count:XX,best_count:[XX,YY,ZZ]}` in the results.
	/// It tracks the number of first calls to `next` that returned an empty set and the number of times the best candidate was either +0, +1, or +2.
	/// defaults to false.
	//enable_statistics: false,
	/// Its name in generated plots.
	legend_name: "Polarized routing",
}
```
**/
#[derive(Debug)]
pub struct Polarized
{
	///Include the weight as label, shifted so that the lowest weight is given the label 0. Otherwise it just put a value of 0 for all.
	include_labels: bool,
	/// Restrict the routes to those that strictly improve the weight function at each step.
	/// Note that mmany/most topologies benefit from using routes that have a few edges with no change to the weight.
	/// Therefore one should expect too few routes when using this option.
	strong: bool,
	// /// Similar to `strong`. A `true` value in `strong_link_classes[c]` indicates links that are considered as candidates only when the weight
	// /// function strictly increases. If the class is out of range then it is considered false.
	// strong_link_classes: Vec<bool>,
	///Whether to raise a panic when there are no candidates. default to true.
	panic_on_empty: bool,
	enable_statistics: bool,
	///The number of first calls to next where the result was empty.
	///enabled by config option `enable_statistics`
	///routing_info.auxiliar counts the calls to next to control it.
	empty_count: Option<RefCell<u64>>,
	///The number of first calls to next for which the best candidate has that mu increment.
	///enabled by config option `enable_statistics`
	best_count: Option<RefCell<[u64;3]>>,
}

impl Routing for Polarized
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, _rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//let (target_location,_link_class)=topology.server_neighbour(target_server);
		//let target_router=match target_location
		//{
		//	Location::RouterPort{router_index,router_port:_} =>router_index,
		//	_ => panic!("The server is not attached to a router"),
		//};
		let distance=topology.distance(current_router,target_router);
		if distance==0
		{
			let target_server = target_server.expect("target server was not given.");
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						let r=(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return Ok(RoutingNextCandidates{candidates:r,idempotent:true});
					}
				}
			}
			unreachable!();
		}
		let source_router=if let Some(ref visited)=routing_info.visited_routers
		{
			visited[0]
		}
		else
		{
			panic!("Unknown source router");
		};
		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);
		let a=topology.distance(source_router,current_router);
		let b=topology.distance(current_router,target_router);
		let weight:i32 = b as i32 - a as i32;
		for port_index in 0..num_ports
		{
			if let (Location::RouterPort{router_index,router_port:_},_link_class)=topology.neighbour(current_router,port_index)
			{
				let new_a=topology.distance(source_router,router_index);
				let new_b=topology.distance(router_index,target_router);
				let new_weight:i32 = new_b as i32 - new_a as i32;
				let condition = new_weight<weight || ( !self.strong
					//&& self.strong_link_classes.get(link_class).map_or(true,|s|!*s)
					&& new_weight==weight && if a<b {a<new_a} else {new_b<b} );
				if condition
				{
					let label=if self.include_labels {new_weight-weight} else {0};//label in {-2,-1,0}. It is shifted later.
					r.extend((0..num_virtual_channels).map(|vc|CandidateEgress{port:port_index,virtual_channel:vc,label,..Default::default()}));
				}
			}
		}
		let call_count: Option<usize> = {
			let mut auxiliar = routing_info.auxiliar.borrow_mut();
			if let Some(any) = &mut *auxiliar {
				let count : &mut usize = any.downcast_mut().expect("auxiliar failed to cast");
				*count += 1;
				//println!("count={count}");
				Some(*count)
			} else {
				None
			}
		};
		if r.is_empty()
		{
			if self.panic_on_empty
			{
				panic!("Polarized routing did not find any candidate output port for s={} c={} t={} a={} b={}",source_router,current_router,target_router,a,b);
			}
			//println!("call_count={call_count:?}");
			if let Some(1) = call_count
			{
				if let Some( empty_count_refcell ) = self.empty_count.as_ref()
				{
					let mut empty_count = empty_count_refcell.borrow_mut();
					*empty_count += 1;
					//println!("empty_count={}",*empty_count);
				}
			}
		}
		//Make the label 0 be the lowest weight variation.
		if let Some(min_label)=r.iter().map(|ref e|e.label).min()
		{
			for ref mut e in r.iter_mut()
			{
				e.label-=min_label;
			}
			if let Some(1) = call_count
			{
				if let Some( best_count_refcell ) = self.best_count.as_ref()
				{
					let mut best_count = best_count_refcell.borrow_mut();
					let index = (-min_label) as usize;
					best_count[index] += 1;
				}
			}
		}
		Ok(RoutingNextCandidates{candidates:r,idempotent:true})
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, current_router:usize, _target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		routing_info.borrow_mut().visited_routers=Some(vec![current_router]);
		if self.enable_statistics
		{
			let any : Box<dyn Any> = Box::new(0usize);
			routing_info.borrow_mut().auxiliar = RefCell::new(Some(any));
		}
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, current_router:usize, _current_port:usize, _target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		let mut ri=routing_info.borrow_mut();
		if let Some(ref mut visited)=ri.visited_routers
		{
			visited.push(current_router);
		}
		if self.enable_statistics
		{
			let any : Box<dyn Any> = Box::new(0usize);
			ri.auxiliar = RefCell::new(Some(any));
		}
	}
	fn initialize(&mut self, topology:&dyn Topology, _rng: &mut StdRng)
	{
		// We only report on whether Polarized is expected to work for a RRG.
		let n=topology.num_routers();
		let eccentricity_vector :Vec<usize> = (0..n).map(|vertex|topology.eccentricity(vertex)).collect();
		let diam = topology.diameter();
		let average_eccentricity = eccentricity_vector.iter().sum::<usize>() as f64 / n as f64;
		let nf = n as f64;
		let max_deg = topology.maximum_degree();
		println!("INFO: n={n} d={max_deg} diameter={diam} average_eccentricity={average_eccentricity}");
		let random_placid_value = (max_deg as f64) / nf.ln() * 2.0f64.ln()/2.0;
		if random_placid_value >= 1.0 {
			println!("INFO: d/ln n * ln 2/2 = {} > 1: In a RRG with these parameters Polarized routing should work.",random_placid_value);
		} else if random_placid_value >= 0.5 {
			println!("INFO: .5 < d/ln n * ln 2/2 = {} < 1: In a RRG with these parameters is not clear whether Polarized routing will have corners.",random_placid_value);
		} else {
			println!("INFO: d/ln n * ln 2/2 = {} < .5: In a RRG with these parameters Polarized routing is expected to have problematic corners.",random_placid_value);
		}
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>, _num_virtual_channels:usize, _rng:&mut StdRng)
	{
	}
	fn statistics(&self, _cycle:Time) -> Option<ConfigurationValue>
	{
		if self.enable_statistics{
			let mut content = Vec::with_capacity(2);
			if let Some(empty_count) = self.empty_count.as_ref()
			{
				content.push(
					(String::from("empty_count"),ConfigurationValue::Number(*empty_count.borrow() as f64))
				);
			}
			if let Some(best_count) = self.best_count.as_ref()
			{
				content.push(
					(String::from("best_count"),ConfigurationValue::Array(
						best_count.borrow().iter().map(|x|ConfigurationValue::Number(*x as f64)).collect()
					))
				);
			}
			Some(ConfigurationValue::Object(String::from("PolarizedStatistics"),content))
		} else {
			None
		}
	}
	fn reset_statistics(&mut self, _next_cycle:Time)
	{
		if self.enable_statistics
		{
			self.empty_count = Some(RefCell::new(0));
			self.best_count = Some(RefCell::new([0,0,0]));
		}
	}
}

impl Polarized
{
	pub fn new(arg:RoutingBuilderArgument) -> Polarized
	{
		let mut include_labels = None;
		let mut strong = None;
		//let mut strong_link_classes = None;
		let mut panic_on_empty = true;
		let mut enable_statistics = false;
		match_object_panic!(arg.cv,"Polarized", value,
			"include_labels" => include_labels=Some(value.as_bool().expect("bad value for include_labels")),
			"strong" => strong=Some(value.as_bool().expect("bad value for strong")),
			//"strong_link_classes" => strong_link_classes=Some(value.as_array().expect("bad value for strong_link_classes").iter()
			//	.map(|item|item.as_bool().expect("bad value for strong_link_classes")
			//).collect()),
			"panic_on_empty" => panic_on_empty=value.as_bool().expect("bad value for panic_on_empty"),
			"enable_statistics" => enable_statistics=value.as_bool().expect("bad value for enable_statistics"),
		);
		//if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		//{
		//	if cv_name!="Polarized"
		//	{
		//		panic!("A Polarized must be created from a `Polarized` object not `{}`",cv_name);
		//	}
		//	for &(ref name,ref value) in cv_pairs
		//	{
		//		//match name.as_ref()
		//		match AsRef::<str>::as_ref(&name)
		//		{
		//			//"full_probability" => match value
		//			//{
		//			//	&ConfigurationValue::Number(f) => full_probability=Some(f as f32),
		//			//	_ => panic!("bad value for full_probability"),
		//			//}
		//			"include_labels" => match value
		//			{
		//				&ConfigurationValue::True => include_labels=Some(true),
		//				&ConfigurationValue::False => include_labels=Some(false),
		//				_ => panic!("bad value for include_labels"),
		//			}
		//			"strong" => match value
		//			{
		//				&ConfigurationValue::True => strong=Some(true),
		//				&ConfigurationValue::False => strong=Some(false),
		//				_ => panic!("bad value for strong"),
		//			}
		//			"panic_on_empty" => match value
		//			{
		//				&ConfigurationValue::True => panic_on_empty=true,
		//				&ConfigurationValue::False => panic_on_empty=false,
		//				_ => panic!("bad value for panic_on_empty"),
		//			}
		//			"enable_statistics" => match value
		//			{
		//				&ConfigurationValue::True => enable_statistics=true,
		//				&ConfigurationValue::False => enable_statistics=false,
		//				_ => panic!("bad value for enable_statistics"),
		//			}
		//			"legend_name" => (),
		//			_ => panic!("Nothing to do with field {} in Polarized",name),
		//		}
		//	}
		//}
		//else
		//{
		//	panic!("Trying to create a Polarized from a non-Object");
		//}
		let include_labels=include_labels.expect("There were no include_labels");
		let strong=strong.unwrap_or_else(||false);
		//let strong_link_classes = strong_link_classes.unwrap_or_else(||Vec::new());
		Polarized{
			include_labels,
			strong,
			//strong_link_classes,
			panic_on_empty,
			enable_statistics,
			empty_count: if enable_statistics {Some(RefCell::new(0))} else {None},
			best_count: if enable_statistics {Some(RefCell::new([0,0,0]))} else {None},
		}
	}
}

