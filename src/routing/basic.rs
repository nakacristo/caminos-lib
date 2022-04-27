
/*!

Implementation of basic routing algorithms.

* Shortest
* Valiant
* Mindless
* WeighedShortest

*/


use std::cell::RefCell;
use ::rand::{rngs::StdRng,Rng};

use crate::match_object_panic;
use crate::config_parser::ConfigurationValue;
use crate::routing::{RoutingBuilderArgument,RoutingInfo,CandidateEgress,RoutingNextCandidates,Routing,new_routing};
use crate::topology::{Topology,Location};
use crate::matrix::Matrix;

///Use the shortest path from origin to destination
#[derive(Debug)]
pub struct Shortest
{
}

impl Routing for Shortest
{
	fn next(&self, _routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, _rng: &RefCell<StdRng>) -> RoutingNextCandidates
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		let distance=topology.distance(current_router,target_router);
		if distance==0
		{
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						//return (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true};
					}
				}
			}
			unreachable!();
		}
		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);
		for i in 0..num_ports
		{
			//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
			if let (Location::RouterPort{router_index,router_port:_},_link_class)=topology.neighbour(current_router,i)
			{
				if distance-1==topology.distance(router_index,target_router)
				{
					//r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)));
					r.extend((0..num_virtual_channels).map(|vc|{
						let mut egress = CandidateEgress::new(i,vc);
						egress.estimated_remaining_hops = Some(distance);
						egress
					}));
				}
			}
		}
		//println!("From router {} to router {} distance={} cand={}",current_router,target_router,distance,r.len());
		RoutingNextCandidates{candidates:r,idempotent:true}
	}
	fn initialize_routing_info(&self, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _rng: &RefCell<StdRng>)
	{
	}
	fn update_routing_info(&self, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _current_port:usize, _target_server:usize, _rng: &RefCell<StdRng>)
	{
	}
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &RefCell<StdRng>)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _num_virtual_channels:usize, _rng:&RefCell<StdRng>)
	{
	}
	fn statistics(&self, _cycle:usize) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:usize)
	{
	}
}

impl Shortest
{
	pub fn new(arg: RoutingBuilderArgument) -> Shortest
	{
		match_object_panic!(arg.cv,"Shortest",_value);
		Shortest{
		}
	}
}

#[derive(Debug)]
pub struct Valiant
{
	first: Box<dyn Routing>,
	second: Box<dyn Routing>,
	///Whether to avoid selecting routers without attached servers. This helps to apply it to indirect networks.
	selection_exclude_indirect_routers: bool,
	first_reserved_virtual_channels: Vec<usize>,
	second_reserved_virtual_channels: Vec<usize>,
}

impl Routing for Valiant
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, rng: &RefCell<StdRng>) -> RoutingNextCandidates
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		let distance=topology.distance(current_router,target_router);
		if distance==0
		{
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						//return (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true}
					}
				}
			}
			unreachable!();
		}
		let meta=routing_info.meta.as_ref().unwrap();
		match routing_info.selections
		{
			None =>
			{
				//self.second.next(&meta[1].borrow(),topology,current_router,target_server,num_virtual_channels,rng)
				let base=self.second.next(&meta[1].borrow(),topology,current_router,target_server,num_virtual_channels,rng);
				let idempotent = base.idempotent;
				let r=base.into_iter().filter(|egress|!self.first_reserved_virtual_channels.contains(&egress.virtual_channel)).collect();
				RoutingNextCandidates{candidates:r,idempotent}
			}
			Some(ref s) =>
			{
				let middle=s[0] as usize;
				let middle_server=
				{
					let mut x=None;
					for i in 0..topology.ports(middle)
					{
						if let (Location::ServerPort(server),_link_class)=topology.neighbour(middle,i)
						{
							x=Some(server);
							break;
						}
					}
					x.unwrap()
				};
				let second_distance=topology.distance(middle,target_router);//Only exact if the base routing is shortest.
				//self.first.next(&meta[0].borrow(),topology,current_router,middle_server,num_virtual_channels,rng).into_iter().filter(|egress|!self.second_reserved_virtual_channels.contains(&egress.virtual_channel)).collect()
				let base = self.first.next(&meta[0].borrow(),topology,current_router,middle_server,num_virtual_channels,rng);
				let idempotent = base.idempotent;
				let r=base.into_iter().filter_map(|mut egress|{
					if self.second_reserved_virtual_channels.contains(&egress.virtual_channel) { None } else {
						if let Some(ref mut eh)=egress.estimated_remaining_hops
						{
							*eh += second_distance;
						}
						Some(egress)
					}
				}).collect();
				RoutingNextCandidates{candidates:r,idempotent}
			}
		}
		// let num_ports=topology.ports(current_router);
		// let mut r=Vec::with_capacity(num_ports*num_virtual_channels);
		// for i in 0..num_ports
		// {
		// 	//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
		// 	if let (Location::RouterPort{router_index,router_port:_},_link_class)=topology.neighbour(current_router,i)
		// 	{
		// 		if distance-1==topology.distance(router_index,target_router)
		// 		{
		// 			r.extend((0..num_virtual_channels).map(|vc|(i,vc)));
		// 		}
		// 	}
		// }
		// //println!("From router {} to router {} distance={} cand={}",current_router,target_router,distance,r.len());
		// r
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_server:usize, rng: &RefCell<StdRng>)
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		let n=topology.num_routers();
		let middle = if self.selection_exclude_indirect_routers
		{
			let available : Vec<usize> = (0..n).filter(|&index|{
				for i in 0..topology.ports(index)
				{
					if let (Location::ServerPort(_),_) = topology.neighbour(index,i)
					{
						return true;
					}
				}
				false//there is not server in this router, hence it is excluded
			}).collect();
			if available.is_empty()
			{
				panic!("There are not legal middle routers to select in Valiant from router {} towards router {}",current_router,target_router);
			}
			//let r = rng.borrow_mut().gen_range(0,available.len());//rand-0.4
			let r = rng.borrow_mut().gen_range(0..available.len());//rand-0.8
			available[r]
		} else {
			rng.borrow_mut().gen_range(0..n)
		};
		let mut bri=routing_info.borrow_mut();
		bri.meta=Some(vec![RefCell::new(RoutingInfo::new()),RefCell::new(RoutingInfo::new())]);
		if middle==current_router || middle==target_router
		{
			self.second.initialize_routing_info(&bri.meta.as_ref().unwrap()[1],topology,current_router,target_server,rng);
		}
		else
		{
			bri.selections=Some(vec![middle as i32]);
			//FIXME: what do we do when we are not excluding indirect routers?
			let middle_server=
			{
				let mut x=None;
				for i in 0..topology.ports(middle)
				{
					if let (Location::ServerPort(server),_link_class)=topology.neighbour(middle,i)
					{
						x=Some(server);
						break;
					}
				}
				x.unwrap()
			};
			self.first.initialize_routing_info(&bri.meta.as_ref().unwrap()[0],topology,current_router,middle_server,rng)
		}
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_server:usize, rng: &RefCell<StdRng>)
	{
		let mut bri=routing_info.borrow_mut();
		let middle = bri.selections.as_ref().map(|s| s[0] as usize);
		match middle
		{
			None =>
			{
				//Already towards true destination
				let meta=bri.meta.as_mut().unwrap();
				meta[1].borrow_mut().hops+=1;
				self.second.update_routing_info(&meta[1],topology,current_router,current_port,target_server,rng);
			}
			Some(middle) =>
			{
				if current_router==middle
				{
					bri.selections=None;
					let meta=bri.meta.as_ref().unwrap();
					self.second.initialize_routing_info(&meta[1],topology,current_router,target_server,rng);
				}
				else
				{
					//FIXME: that target_server
					let meta=bri.meta.as_mut().unwrap();
					meta[0].borrow_mut().hops+=1;
					self.first.update_routing_info(&meta[0],topology,current_router,current_port,target_server,rng);
				}
			}
		};
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &RefCell<StdRng>)
	{
		self.first.initialize(topology,rng);
		self.second.initialize(topology,rng);
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _num_virtual_channels:usize, _rng:&RefCell<StdRng>)
	{
		//TODO: recurse over routings
	}
	fn statistics(&self, _cycle:usize) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:usize)
	{
	}
}

impl Valiant
{
	pub fn new(arg: RoutingBuilderArgument) -> Valiant
	{
		//let mut order=None;
		//let mut servers_per_router=None;
		let mut first=None;
		let mut second=None;
		let mut selection_exclude_indirect_routers=false;
		let mut first_reserved_virtual_channels=vec![];
		let mut second_reserved_virtual_channels=vec![];
		match_object_panic!(arg.cv,"Valiant",value,
			"first" => first=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"second" => second=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"selection_exclude_indirect_routers" => selection_exclude_indirect_routers = value.as_bool().expect("bad value for selection_exclude_indirect_routers"),
			"first_reserved_virtual_channels" => first_reserved_virtual_channels=value.
				as_array().expect("bad value for first_reserved_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in first_reserved_virtual_channels") as usize).collect(),
			"second_reserved_virtual_channels" => second_reserved_virtual_channels=value.
				as_array().expect("bad value for second_reserved_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in second_reserved_virtual_channels") as usize).collect(),
		);
		let first=first.expect("There were no first");
		let second=second.expect("There were no second");
		//let first_reserved_virtual_channels=first_reserved_virtual_channels.expect("There were no first_reserved_virtual_channels");
		//let second_reserved_virtual_channels=second_reserved_virtual_channels.expect("There were no second_reserved_virtual_channels");
		Valiant{
			first,
			second,
			selection_exclude_indirect_routers,
			first_reserved_virtual_channels,
			second_reserved_virtual_channels,
		}
	}
}


///Mindless routing
///Employ any path until reaching a router with the server atached.
///The interested may read a survey of random walks on graphs to try to predict the time to reach the destination. For example "Random Walks on Graphs: A Survey" by L. Lovász.
///Note that every cycle the request is made again. Hence, the walk is not actually unform random when there is network contention.
#[derive(Debug)]
pub struct Mindless
{
}

impl Routing for Mindless
{
	fn next(&self, _routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, _rng: &RefCell<StdRng>) -> RoutingNextCandidates
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		if target_router==current_router
		{
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						//return (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true}
					}
				}
			}
			unreachable!();
		}
		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);
		for i in 0..num_ports
		{
			//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
			if let (Location::RouterPort{router_index:_,router_port:_},_link_class)=topology.neighbour(current_router,i)
			{
				//r.extend((0..num_virtual_channels).map(|vc|(i,vc)));
				r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)));
			}
		}
		RoutingNextCandidates{candidates:r,idempotent:true}
	}
	fn initialize_routing_info(&self, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _rng: &RefCell<StdRng>)
	{
	}
	fn update_routing_info(&self, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _current_port:usize, _target_server:usize, _rng: &RefCell<StdRng>)
	{
	}
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &RefCell<StdRng>)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _num_virtual_channels:usize, _rng:&RefCell<StdRng>)
	{
	}
	fn statistics(&self, _cycle:usize) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:usize)
	{
	}
}

impl Mindless
{
	pub fn new(arg: RoutingBuilderArgument) -> Mindless
	{
		match_object_panic!(arg.cv,"Mindless",_value);
		Mindless{
		}
	}
}

///Use the shortest path from origin to destination, giving a weight to each link class.
///Note that it uses information based on BFS and not on Dijkstra, which may cause discrepancies in some topologies.
///See the `Topology::compute_distance_matrix` and its notes on weights for more informations.
#[derive(Debug)]
pub struct WeighedShortest
{
	///The weights used for each link class. Only relevant links between routers.
	class_weight:Vec<usize>,
	///The distance matrix computed, including weights.
	distance_matrix: Matrix<usize>,
}

impl Routing for WeighedShortest
{
	fn next(&self, _routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, _rng: &RefCell<StdRng>) -> RoutingNextCandidates
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		//let distance=topology.distance(current_router,target_router);
		let distance=*self.distance_matrix.get(current_router,target_router);
		//let valid = vec![0,1,2,100,101,102];
		//if !valid.contains(&distance){ panic!("distance={}",distance); }
		if distance==0
		{
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						//return (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true};
					}
				}
			}
			unreachable!();
		}
		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);
		for i in 0..num_ports
		{
			//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
			if let (Location::RouterPort{router_index,router_port:_},link_class)=topology.neighbour(current_router,i)
			{
				let link_weight = self.class_weight[link_class];
				//if distance>*self.distance_matrix.get(router_index,target_router)
				let new_distance = *self.distance_matrix.get(router_index,target_router);
				if new_distance + link_weight == distance
				{
					//if ![(102,1),(1,1),(101,100),(100,100),(101,1)].contains(&(distance,link_weight)){
					//	println!("distance={} link_weight={}",distance,link_weight);
					//}
					//println!("distance={} link_weight={} hops={}",distance,link_weight,routing_info.hops);
					//r.extend((0..num_virtual_channels).map(|vc|(i,vc)));
					r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)));
				}
			}
		}
		//println!("From router {} to router {} distance={} cand={}",current_router,target_router,distance,r.len());
		RoutingNextCandidates{candidates:r,idempotent:true}
	}
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, toology:&dyn Topology, current_router:usize, target_server:usize)
	fn initialize_routing_info(&self, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _rng: &RefCell<StdRng>)
	{
	}
	fn update_routing_info(&self, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _current_port:usize, _target_server:usize, _rng: &RefCell<StdRng>)
	{
	}
	fn initialize(&mut self, topology:&dyn Topology, _rng: &RefCell<StdRng>)
	{
		self.distance_matrix=topology.compute_distance_matrix(Some(&self.class_weight));
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _num_virtual_channels:usize, _rng:&RefCell<StdRng>)
	{
	}
	fn statistics(&self, _cycle:usize) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:usize)
	{
	}
}

impl WeighedShortest
{
	pub fn new(arg: RoutingBuilderArgument) -> WeighedShortest
	{
		let mut class_weight=None;
		match_object_panic!(arg.cv,"WeighedShortest",value,
			"class_weight" => class_weight = Some(value.as_array()
				.expect("bad value for class_weight").iter()
				.map(|v|v.as_f64().expect("bad value in class_weight") as usize).collect()),
		);
		let class_weight=class_weight.expect("There were no class_weight");
		WeighedShortest{
			class_weight,
			distance_matrix:Matrix::constant(0,0,0),
		}
	}
}

