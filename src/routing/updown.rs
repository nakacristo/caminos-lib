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
	up_down_distances: Matrix<Option<u8>>,
	down_distances: Matrix<Option<u8>>,
	distance_to_root: Vec<u8>,
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
		if current_router == target_router
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
		let up_down_distance = self.up_down_distances.get(current_router,target_router).unwrap_or_else(||panic!("Missing up/down path from {} to {}",current_router,target_router));
		let down_distance = self.down_distances.get(current_router,target_router);
		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);
		for i in 0..num_ports
		{
			//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
			if let (Location::RouterPort{router_index,router_port:_},_link_class)=topology.neighbour(current_router,i)
			{
				let good = if let Some(down_distance) = down_distance {
					//We can already go down
					if let Some(new_down) = self.down_distances.get(router_index,target_router) {
						new_down < down_distance
					} else {
						false
					}
				} else {
					if let &Some(new_up_down) = self.up_down_distances.get(router_index,target_router)
					{
						//force up
						new_up_down < up_down_distance && self.distance_to_root[router_index]<self.distance_to_root[current_router]
					} else {
						false
					}
				};
				if good{
					r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)));
				}
			}
		}
		//println!("candidates={:?} current_router={current_router} target_router={target_router} up_down_distance={up_down_distance} down_distance={down_distance:?}",r.iter().map(|x|x.port).collect::<Vec<_>>());
		//println!("From router {} to router {} distance={} cand={}",current_router,target_router,distance,r.len());
		RoutingNextCandidates{candidates:r,idempotent:true}
	}
	fn initialize(&mut self, topology:&dyn Topology, _rng: &RefCell<StdRng>)
	{
		let n = topology.num_routers();
		if let Some(root) = self.root
		{
			self.up_down_distances = Matrix::constant(None,n,n);
			self.down_distances = Matrix::constant(None,n,n);
			//First perform a single BFS at root.
			let mut distance_to_root=vec![None;n];
			distance_to_root[root]=Some(0);
			//A BFS from the root.
			let mut downwards = Vec::with_capacity(n);
			let mut read_index = 0;
			downwards.push(root);
			//for current in 0..n
			while read_index < downwards.len()
			{
				let current = downwards[read_index];
				read_index+=1;
				if let Some(current_distance) = distance_to_root[current]
				{
					let alternate_distance = current_distance + 1;
					for NeighbourRouterIteratorItem{neighbour_router:neighbour,..} in topology.neighbour_router_iter(current)
					{
						if distance_to_root[neighbour].is_none()
						{
							distance_to_root[neighbour]=Some(alternate_distance);
							downwards.push(neighbour);
						}
					}
				}
			}
			self.distance_to_root = distance_to_root.into_iter().map(|d|d.unwrap()).collect();
			//dbg!(&distance_to_root);
			//Second fill assuming going through root
			//dbg!(root,"fill");
			for origin in 0..n
			{
				let origin_to_root = self.distance_to_root[origin];
				for target in 0..n
				{
					let target_to_root = self.distance_to_root[target];
					*self.up_down_distances.get_mut(origin,target) = Some(origin_to_root+target_to_root);
				}
				*self.down_distances.get_mut(root,origin) = Some(origin_to_root);
			}
			//Update the distances considering not reaching the root.
			for origin in 0..n
			{
				*self.up_down_distances.get_mut(origin,origin) = Some(0);
				*self.down_distances.get_mut(origin,origin) = Some(0);
			}
			//dbg!(root,"segments");
			//As invariant: fully computed the higher part (closer to the root).
			for (low_index,&low) in downwards.iter().enumerate()
			{
				for &high in downwards[0..low_index].iter()
				{
					for NeighbourRouterIteratorItem{neighbour_router:neighbour,..} in topology.neighbour_router_iter(low)
					{
						if self.distance_to_root[neighbour]+1==self.distance_to_root[low]
						{
							//neighbour is upwards
							let neighbour_up_down = self.up_down_distances.get(neighbour,high).unwrap();
							let origin_up_down = self.up_down_distances.get(low,high).unwrap();
							if neighbour_up_down+1 < origin_up_down
							{
								*self.up_down_distances.get_mut(low,high) = Some(neighbour_up_down+1);
								*self.up_down_distances.get_mut(high,low) = Some(neighbour_up_down+1);
							}
							if let Some(neighbour_down) = self.down_distances.get(high,neighbour)
							{
								if self.down_distances.get(high,low).map(|origin_down|neighbour_down+1<origin_down).unwrap_or(true)
								{
									//println!("high={high} neighbour={neighbour} low={low} distance={}",neighbour_down+1);
									*self.down_distances.get_mut(high,low) = Some(neighbour_down+1);
								}
							}
						}
					}
				}
			}
			//dbg!(&self.up_down_distances);
			//for origin in 0..n
			//{
			//	//Start towards root annotating those that require only upwards.
			//	//let _origin_to_root) = distance_to_root[origin];
			//	let mut upwards=Vec::with_capacity(n);
			//	upwards.push((origin,0));
			//	let mut read_index = 0;
			//	while read_index < upwards.len()
			//	{
			//		let (current,distance) = upwards[read_index];
			//		let current_to_root = distance_to_root[current];
			//		read_index+=1;
			//		*self.up_down_distances.get_mut(origin,current)=Some((distance,0));
			//		*self.up_down_distances.get_mut(current,origin)=Some((0,distance));
			//		for NeighbourRouterIteratorItem{neighbour_router:neighbour,..} in topology.neighbour_router_iter(current)
			//		{
			//			let neighbour_to_root = distance_to_root[neighbour];
			//			if neighbour_to_root +1 == current_to_root
			//			{
			//				upwards.push((neighbour,distance+1));
			//			}
			//		}
			//	}
			//}
			//dbg!(root,"finished table");
		}
		if n!=self.up_down_distances.get_columns()
		{
			panic!("ExplicitUpDown has not being properly initialized");
		}
	}
	//fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _rng: &RefCell<StdRng>)
	//{
	//	routing_info.borrow_mut().selections=Some(Vec::new());
	//}
	//fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, _current_port:usize, _target_server:usize, _rng: &RefCell<StdRng>)
	//{
	//	let mut bri = routing_info.borrow_mut();
	//	let v = bri.selections.as_mut().unwrap();
	//	let root = *self.root.as_ref().unwrap();
	//	let distance = topology.distance(root,current_router);
	//	v.push(distance as i32);
	//	println!("distances={v:?} current_router={current_router}");
	//}
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
			down_distances: Matrix::constant(None,0,0),
			distance_to_root: Vec::new(),
		}
	}
}

