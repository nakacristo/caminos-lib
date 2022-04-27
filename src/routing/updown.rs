/*!

Implementation of general Up/Down-like routings.

* UpDown
* UpDownStar (struct ExplicitUpDown)

*/

use std::cell::RefCell;
use ::rand::{rngs::StdRng};

use crate::match_object_panic;
use crate::config_parser::ConfigurationValue;
use crate::routing::{RoutingBuilderArgument,RoutingInfo,CandidateEgress,RoutingNextCandidates,Routing};
use crate::topology::{Topology,NeighbourRouterIteratorItem,Location};
use crate::matrix::Matrix;

///Use a shortest up/down path from origin to destination.
///The up/down paths are understood as provided by `Topology::up_down_distance`.
#[derive(Debug)]
pub struct UpDown
{
}

impl Routing for UpDown
{
	fn next(&self, _routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, _rng: &RefCell<StdRng>) -> RoutingNextCandidates
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		let (up_distance, down_distance) = topology.up_down_distance(current_router,target_router).unwrap_or_else(||panic!("The topology does not provide an up/down path from {} to {}",current_router,target_router));
		if up_distance + down_distance == 0
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
				if let Some((new_u, new_d)) = topology.up_down_distance(router_index,target_router)
				{
					if (new_u<up_distance && new_d<=down_distance) || (new_u<=up_distance && new_d<down_distance)
					{
						r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)));
					}
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
	fn initialize(&mut self, _topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)
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

impl UpDown
{
	pub fn new(arg: RoutingBuilderArgument) -> UpDown
	{
		match_object_panic!(arg.cv,"UpDown",_value);
		UpDown{
		}
	}
}

///Use a shortest up/down path from origin to destination.
///But in contrast with UpDown this uses explicit table instead of querying the topology.
///Used to define Up*/Down* (UpDownStar), see Autonet, where it is build from some spanning tree.
#[derive(Debug)]
pub struct ExplicitUpDown
{
	//defining factors to be kept up to initialization
	root: Option<usize>,
	//computed at initialization
	up_down_distances: Matrix<Option<(u8,u8)>>,
}

impl Routing for ExplicitUpDown
{
	fn next(&self, _routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, _rng: &RefCell<StdRng>) -> RoutingNextCandidates
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		let (up_distance, down_distance) = self.up_down_distances.get(current_router,target_router).unwrap_or_else(||panic!("Missing up/down path from {} to {}",current_router,target_router));
		if up_distance + down_distance == 0
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
				if let &Some((new_u, new_d)) = self.up_down_distances.get(router_index,target_router)
				{
					if (new_u<up_distance && new_d<=down_distance) || (new_u<=up_distance && new_d<down_distance)
					{
						r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)));
					}
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
	fn initialize(&mut self, topology:&Box<dyn Topology>, _rng: &RefCell<StdRng>)
	{
		let n = topology.num_routers();
		if let Some(root) = self.root
		{
			self.up_down_distances = Matrix::constant(None,n,n);
			//First perform a single BFS at root.
			let mut distance_to_root=vec![None;n];
			distance_to_root[root]=Some(0);
			//The updwards BFS.
			dbg!(root,"upwards");
			for current in 0..n
			{
				if let Some(current_distance) = distance_to_root[current]
				{
					let alternate_distance = current_distance + 1;
					for NeighbourRouterIteratorItem{neighbour_router:neighbour,..} in topology.neighbour_router_iter(current)
					{
						if distance_to_root[neighbour].is_none()
						{
							distance_to_root[neighbour]=Some(alternate_distance);
						}
					}
				}
			}
			//Second fill assuming going through root
			dbg!(root,"fill");
			for origin in 0..n
			{
				if let Some(origin_to_root) = distance_to_root[origin]
				{
					for target in 0..n
					{
						if let Some(target_to_root) = distance_to_root[target]
						{
							*self.up_down_distances.get_mut(origin,target) = Some((origin_to_root,target_to_root));
						}
					}
				}
			}
			//Now fix all little segments that do not reach the root.
			dbg!(root,"segments");
			for origin in 0..n
			{
				//Start towards root annotating those that require only upwards.
				if let Some(_origin_to_root) = distance_to_root[origin]
				{
					let mut upwards=Vec::with_capacity(n);
					upwards.push((origin,0));
					let mut read_index = 0;
					while read_index < upwards.len()
					{
						let (current,distance) = upwards[read_index];
						if let Some(current_to_root) = distance_to_root[current]
						{
							read_index+=1;
							*self.up_down_distances.get_mut(origin,current)=Some((distance,0));
							*self.up_down_distances.get_mut(current,origin)=Some((0,distance));
							for NeighbourRouterIteratorItem{neighbour_router:neighbour,..} in topology.neighbour_router_iter(current)
							{
								if let Some(neighbour_to_root) = distance_to_root[neighbour]
								{
									if neighbour_to_root +1 == current_to_root
									{
										upwards.push((neighbour,distance+1));
									}
								}
							}
						}
					}
				}
			}
			dbg!(root,"finished table");
		}
		if n!=self.up_down_distances.get_columns()
		{
			panic!("ExplicitUpDown has not being properly initialized");
		}
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

impl ExplicitUpDown
{
	pub fn new(arg: RoutingBuilderArgument) -> ExplicitUpDown
	{
		let mut root = None;
		match_object_panic!(arg.cv,"UpDownStar",value,
			"root" => root=Some(value.as_f64().expect("bad value for root") as usize),
		);
		ExplicitUpDown{
			root,
			up_down_distances: Matrix::constant(None,0,0),
		}
	}
}

