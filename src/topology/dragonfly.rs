
use ::rand::{rngs::StdRng};
use super::prelude::*;
use super::cartesian::CartesianData;
use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::matrix::Matrix;
use crate::quantify::Quantifiable;
use crate::match_object_panic;
use crate::pattern::prelude::*;
use crate::pattern::UniformPattern;

/**
Builds a dragonfly topology, this is, a hierarchical topology where each group is fully-connected (a complete graph) and each pair of groups is connected at least with a global link.
 There are several possible arrangements for the global links, by default it uses the palm-tree arrangement.
The canonic dimensions (the CanonicDragonfly name has been deprecated) are
* to have as many global links as links to servers in each router,
* to have in each group the double number of routers than links to a server in a router (this point is taken by default if not given),
* to have a unique global link joining each pair of groups,
* and to have a unique local link joining each pair of router in the same group.
For the palm-tree arrangement we refer to the doctoral thesis of Marina García.

For the palmtree arrangement exteded to other size ratios and the [Dragonfly2ColorsRouting] routing see
Cristóbal Camarero, Enrique Vallejo, and Ramón Beivide. 2014. Topological Characterization of Hamming and Dragonfly Networks and Its Implications on Routing. ACM Trans. Archit. Code Optim. 11, 4, Article 39 (January 2015), 25 pages. https://doi.org/10.1145/2677038

Example configuration:
```ignore
Dragonfly{
	/// Number of ports per router that connect to routers in a different group. Dally called it `h`
	global_ports_per_router: 4,
	/// Number of servers per router. Dally called it `p`. Typically p=h.
	servers_per_router: 4,
	/// Configuration of the global links.
	global_arrangement: Random,
	/// Number of routers in a group. Dally called it `a`. a-1 local ports. Defaults to the canonic dragonfly, i.e.,  a=2h.
	//group_size: 8,
	/// Number of groups. Denoted by `g` in Dally's paper. Defaults to the canonic dragonfly value of `g = a*h+1`.
	//number_of_groups: 10,
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Dragonfly
{
	/// Number of ports per router that connect to routers in a different group. Dally called it `h`
	global_ports_per_router: usize,
	/// Number of servers per router. Dally called it `p`. Typically p=h.
	servers_per_router: usize,
	/// Configuration of the global links.
	global_arrangement: Box<dyn Arrangement>,
	/// Number of routers in a group. Dally called it `a`. a-1 local ports. In a canonic dragonfly a=2h.
	group_size: usize,
	/// Number of groups. Denoted by `g` in Dally's paper. In a canonic dragonfly `g = a*h+1`.
	number_of_groups: usize,

	// cached values:
	// Cartesian data [switch_index, group_index]
	cartesian_data: CartesianData,
	/// `distance_matrix.get(i,j)` = distance from router i to router j.
	distance_matrix:Matrix<u8>,
}

impl Topology for Dragonfly
{
	fn num_routers(&self) -> usize
	{
		self.group_size * self.number_of_groups
	}
	fn num_servers(&self) -> usize
	{
		self.group_size * self.number_of_groups * self.servers_per_router
	}
	fn neighbour(&self, router_index:usize, port: usize) -> (Location,usize)
	{
		let (router_local,router_global)=self.unpack(router_index);
		let degree=self.group_size-1+self.global_ports_per_router;
		if port<self.group_size-1
		{
			let target_local = (router_local+1+port)%self.group_size;
			let target_port = self.group_size - 2 - port;
			//println!("{},{} l{} -> {},{} l{}",router_local,router_global,port,target_local,router_global,target_port);
			(Location::RouterPort{router_index:self.pack((target_local,router_global)),router_port:target_port},0)
		}
		else if port<degree
		{
			// XXX Assuming palmtree for now
			// let port_offset=port+1-self.group_size;
			// let target_global=(router_global+self.number_of_groups-(router_local*self.global_ports_per_router+port_offset+1)) % self.number_of_groups;
			// let target_local=( ((self.number_of_groups+target_global-router_global)%self.number_of_groups)-1 )/self.global_ports_per_router;
			// let target_port=self.group_size-1  +  self.global_ports_per_router-1-port_offset;
			let point = ArrangementPoint {
				group_index: router_global,
				group_offset: router_local,
				port_index: port + 1-self.group_size,//substract the ports before global ports
			};
			//assert!(size.contains(point), "arrangement point {:?} is not in range. size is {:?}",point,size);
			let target_point = self.global_arrangement.map(point);
			let target_global = target_point.group_index;
			let target_local = target_point.group_offset;
			let target_port = (self.group_size-1) + target_point.port_index;
			//println!("{},{} g{} -> {},{} g{}",router_local,router_global,port_offset,target_local,target_global,target_port+1-self.group_size);
			(Location::RouterPort{router_index:self.pack((target_local,target_global)),router_port:target_port},1)
		}
		else
		{
			(Location::ServerPort(router_index*self.servers_per_router + port-degree),2)
		}
	}
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let degree = self.group_size-1 + self.global_ports_per_router;
		(Location::RouterPort{
			router_index: server_index/self.servers_per_router,
			router_port: degree + server_index%self.servers_per_router,
		},2)
	}
	fn diameter(&self) -> usize
	{
		3
	}
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		(*self.distance_matrix.get(origin,destination)).into()
	}
	fn amount_shortest_paths(&self,_origin:usize,_destination:usize) -> usize
	{
		//*self.amount_matrix.get(origin,destination)
		unimplemented!();
	}
	fn average_amount_shortest_paths(&self) -> f32
	{
		//self.average_amount
		unimplemented!();
	}
	fn maximum_degree(&self) -> usize
	{
		self.group_size-1 + self.global_ports_per_router
	}
	fn minimum_degree(&self) -> usize
	{
		self.group_size-1 + self.global_ports_per_router
	}
	fn degree(&self, _router_index: usize) -> usize
	{
		self.group_size-1 + self.global_ports_per_router
	}
	fn ports(&self, _router_index: usize) -> usize
	{
		self.group_size-1 + self.global_ports_per_router + self.servers_per_router
	}
	fn cartesian_data(&self) -> Option<&CartesianData>
	{
		Some(&self.cartesian_data)
	}
	fn coordinated_routing_record(&self, _coordinates_a:&[usize], _coordinates_b:&[usize], _rng: Option<&mut StdRng>)->Vec<i32>
	{
		//(0..coordinates_a.len()).map(|i|coordinates_b[i] as i32-coordinates_a[i] as i32).collect()
		unimplemented!();
	}
	fn is_direction_change(&self, _router_index:usize, _input_port: usize, _output_port: usize) -> bool
	{
		//input_port/2 != output_port/2
		true
	}
	fn up_down_distance(&self,_origin:usize,_destination:usize) -> Option<(usize,usize)>
	{
		None
	}
	fn dragonfly_size(&self) -> Option<ArrangementSize> {
		Some(ArrangementSize{
			number_of_groups: self.number_of_groups,
			group_size: self.group_size,
			number_of_ports: self.global_ports_per_router,
		})
	}
}

impl Dragonfly
{
	pub fn new(arg:TopologyBuilderArgument) -> Dragonfly
	{
		let mut global_ports_per_router=None;
		let mut servers_per_router=None;
		let mut global_arrangement=None;
		let mut group_size=None;
		let mut number_of_groups = None;
		match_object_panic!(arg.cv,["Dragonfly", "CanonicDragonfly"],value,
			"global_ports_per_router" => global_ports_per_router=Some(value.as_f64().expect("bad value for global_ports_per_router")as usize),
			"servers_per_router" => servers_per_router=Some(value.as_f64().expect("bad value for servers_per_router")as usize),
			"global_arrangement" => global_arrangement=Some(new_arrangement(value.into())),
			"group_size" => group_size=Some(value.as_usize().expect("bad value for group_size")),
			"number_of_groups" => number_of_groups=Some(value.as_usize().expect("bad value for number_of_groups")),
		);
		let global_ports_per_router=global_ports_per_router.expect("There were no global_ports_per_router");
		let servers_per_router=servers_per_router.expect("There were no servers_per_router");
		let group_size = group_size.unwrap_or_else(||2*global_ports_per_router);
		let number_of_groups = number_of_groups.unwrap_or_else(||group_size*global_ports_per_router + 1);
		let mut global_arrangement = global_arrangement.unwrap_or_else(||Box::new(Palmtree::default()));
		global_arrangement.initialize(ArrangementSize{
			number_of_groups,
			group_size,
			number_of_ports: global_ports_per_router,
		},arg.rng);
		let mut topo=Dragonfly{
			global_ports_per_router,
			servers_per_router,
			global_arrangement,
			group_size,
			number_of_groups,
			distance_matrix:Matrix::constant(0,0,0),
			cartesian_data: CartesianData::new(&vec![group_size, number_of_groups]),
		};
		let (distance_matrix,_amount_matrix)=topo.compute_amount_shortest_paths();
		topo.distance_matrix=distance_matrix.map(|x|*x as u8);
		topo
	}
	/**
	 Unpack a switch index into `(group_offset, group_index)` coordinates.
	 With `group_offset` beings the position of the switch in the group and `group_index` the index of the group.
	**/
	fn unpack(&self, router_index: usize) -> (usize,usize)
	{
		(router_index%self.group_size,router_index/self.group_size)
	}
	/**
	 Pack coordinates `(group_offset, group_index)` into a whole switch index.
	**/
	fn pack(&self, coordinates:(usize,usize)) -> usize
	{
		coordinates.0+coordinates.1*self.group_size
	}
}

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct ArrangementPoint
{
	/// Which group.
	pub group_index: usize,
	/// Position inside group.
	pub group_offset: usize,
	/// A global port of the switch.
	pub port_index: usize,
}

#[derive(Clone,Copy,Debug,Default,Quantifiable)]
pub struct ArrangementSize
{
	pub number_of_groups: usize,
	pub group_size: usize,
	pub number_of_ports: usize,
}

impl ArrangementSize
{
	pub fn contains(self, point:ArrangementPoint) -> bool
	{
		(0..self.number_of_groups).contains(&point.group_index)
		&& (0..self.group_size).contains(&point.group_offset)
		&& (0..self.number_of_ports).contains(&point.port_index)
	}
	/// Like the method in [Dragonfly].
	fn unpack(&self, router_index: usize) -> (usize,usize)
	{
		(router_index%self.group_size,router_index/self.group_size)
	}
	/// Like the method in [Dragonfly].
	fn pack(&self, coordinates:(usize,usize)) -> usize
	{
		coordinates.0+coordinates.1*self.group_size
	}
}

/**
An arrangement represents the map of global ports of a Dragonfly-like networks by its global links.
It is called a point to a combination of group, router, and port identifier.
**/
pub trait Arrangement : Quantifiable + core::fmt::Debug
{
	/// Initialization should be called once before any other of its methods.
	fn initialize(&mut self, size:ArrangementSize, rng: &mut StdRng);
	/// Gets the point connected to the `input`.
	fn map( &self, input:ArrangementPoint ) -> ArrangementPoint;
	/// Get the size with the arrangement has been initialized.
	fn get_size(&self) -> ArrangementSize;
	/// Checks whether the arrangement is involution and in range
	fn is_valid( &self ) -> bool
	{
		let size = self.get_size();
		for group_index in 0..size.number_of_groups
		{
			for group_offset in 0..size.group_size
			{
				for port_index in 0..size.number_of_ports
				{
					let input = ArrangementPoint{group_index,group_offset,port_index};
					let target = self.map(input);
					if !size.contains(target) { return false }//has to be in range
					let back = self.map(target);
					if input != back { return false }//has to be an involution
				}
			}
		}
		true
	}
	/// For each pair of groups count the number of links.
	fn global_trunking_distribution( &self ) -> Matrix<usize>
	{
		let size = self.get_size();
		let mut result : Matrix<usize> = Matrix::constant(0,size.number_of_groups,size.number_of_groups);
		for group_index in 0..size.number_of_groups
		{
			for group_offset in 0..size.group_size
			{
				for port_index in 0..size.number_of_ports
				{
					let input = ArrangementPoint{group_index,group_offset,port_index};
					let target = self.map(input);
					*result.get_mut(input.group_index,target.group_index)+=1;
				}
			}
		}
		return result;
	}
}

/// Marina García's regular arrangement for the dragonfly.
/// Only works for `number_of_groups=group_size*number_of_ports+1`.
#[derive(Quantifiable,Debug,Default)]
pub struct Palmtree
{
	size: ArrangementSize,
}

impl Arrangement for Palmtree
{
	fn initialize(&mut self, size:ArrangementSize, _rng: &mut StdRng)
	{
		self.size = size;
	}
	fn map( &self, input:ArrangementPoint ) -> ArrangementPoint
	{
		// old for just canonical sizes
		//let target_group_index = (
		//	input.group_index
		//	+ self.size.number_of_groups//to ensure being positive
		//	- (input.group_offset*self.size.number_of_ports+input.port_index+1)
		//) % self.size.number_of_groups;
		//let target_group_offset=(
		//	((self.size.number_of_groups+target_group_index-input.group_index)%self.size.number_of_groups) - 1
		//) / self.size.number_of_ports;
		// extended, for other sizes. tested by extended_palmtree
		let target_group_offset = self.size.group_size - input.group_offset - 1;
		let target_port = self.size.number_of_ports-1-input.port_index;
		let target_group_index = (
			input.group_index+1+
				((target_group_offset)*self.size.number_of_ports+target_port) % (self.size.number_of_groups-1)
		) % self.size.number_of_groups;
		ArrangementPoint{
			group_index: target_group_index,
			group_offset: target_group_offset,
			port_index: target_port,
		}
	}
	fn get_size(&self) -> ArrangementSize
	{
		self.size
	}
}

#[derive(Quantifiable,Debug,Default)]
pub struct RandomArrangement
{
	size: ArrangementSize,
	inner_map: Vec<usize>,
}

impl Arrangement for RandomArrangement
{
	fn initialize(&mut self, size:ArrangementSize, rng: &mut StdRng)
	{
		use rand::prelude::SliceRandom;
		use rand::Rng;
		self.size = size;
		let n = size.number_of_groups;
		let m = size.group_size*size.number_of_ports;
		let group_pairs = n*(n-1)/2;
		let total_points = n*m;
		let base_trunking = total_points/2 / group_pairs;
		let irregular_links = total_points/2 - base_trunking*group_pairs;
		let mut free_points : Vec<Vec<usize>> = (0..n).map(|_| (0..m).collect() ).collect();
		self.inner_map = vec![0;total_points];
		for _ in 0..base_trunking
		{
			// Add one random connection to every pair of groups.
			let mut order = Vec::with_capacity(n*(n-1)/2);
			for i in 0..n
			{
				for j in (i+1)..n
				{
					order.push( (i,j) );
				}
			}
			order.shuffle(&mut*rng);
			for (group_left,group_right) in order
			{
				// Get a random free point in each group
				let left_selection = rng.gen_range( 0..free_points[group_left].len() );
				let right_selection = rng.gen_range( 0..free_points[group_right].len() );
				let left_point = free_points[group_left][left_selection] + group_left*m;
				let right_point = free_points[group_right][right_selection] + group_right*m;
				free_points[group_left].swap_remove(left_selection);
				free_points[group_right].swap_remove(right_selection);
				self.inner_map[left_point] = right_point;
				self.inner_map[right_point] = left_point;
			}
		}
		if irregular_links>0
		{
			// Randomly connects pairs of free points.
			let mut free_points : Vec<usize> = free_points.iter().enumerate().flat_map(
				|(group,points)| points.iter().map( move |&p|group*m+p)
			).collect();
			while free_points.len() > 0
			{
				let first_selection = rng.gen_range( 0..free_points.len() );
				let first = free_points[first_selection];
				free_points.swap_remove(first_selection);
				let second_selection = rng.gen_range( 0..free_points.len() );
				let second = free_points[second_selection];
				free_points.swap_remove(second_selection);
				self.inner_map[first] = second;
				self.inner_map[second] = first;
			}
		}
	}
	fn map( &self, input:ArrangementPoint ) -> ArrangementPoint
	{
		let input_flat = input.port_index + self.size.number_of_ports*(input.group_offset + self.size.group_size*input.group_index);
		let output_flat = self.inner_map[input_flat];
		let output_group = output_flat / (self.size.number_of_ports*self.size.group_size);
		let output_offset = (output_flat / self.size.number_of_ports) % self.size.group_size;
		let output_port = output_flat % self.size.number_of_ports;
		ArrangementPoint{
			group_index: output_group,
			group_offset: output_offset,
			port_index: output_port,
		}
	}
	fn get_size(&self) -> ArrangementSize
	{
		self.size
	}
}

pub struct ArrangementBuilderArgument<'a>
{
	pub cv: &'a ConfigurationValue,
}

impl<'a> From<&'a ConfigurationValue> for ArrangementBuilderArgument<'a>
{
	fn from(cv:&'a ConfigurationValue) -> Self
	{
		ArrangementBuilderArgument{cv}
	}
}

pub fn new_arrangement(arg:ArrangementBuilderArgument) -> Box<dyn Arrangement>
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
	{
		//if let Some(builder) = arg.plugs.topologies.get(cv_name)
		//{
		//	return builder(arg);
		//}
		match cv_name.as_ref()
		{
			"Palmtree" => Box::new(Palmtree::default()),
			"Random" => Box::new(RandomArrangement::default()),
			_ => panic!("Unknown arrangement {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create an arrangement from a non-Object");
	}
}


use crate::routing::prelude::*;

/**
With the switches colored in {0,1} with a global arrangement such that global links connect only switches of the same color, the global link is labelled by that color.
The local links are labelled being either +0 or +1: +0 for links connecting switches of same color and +1 for links connecting switches of different color.
This routing employs routes lgl, where some hops may be skipped.
If source and destination have different color then use L+0, G0, L+1. If they have same color then
- L+0, G0, L+0 when the group of source has lower offest than the destination group.
- L+1, G1, L+1 when the group of source has greater offset than the destination group.
- If they are in the same group is just a local link of either kind.
This routing does not require virtual channels.
**/
#[derive(Debug)]
pub struct Dragonfly2ColorsRouting
{
}

impl Routing for Dragonfly2ColorsRouting
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, _rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//let (target_location,_link_class)=topology.server_neighbour(target_server);
		//let target_router=match target_location
		//{
		//	Location::RouterPort{router_index,router_port:_} =>router_index,
		//	_ => panic!("The server is not attached to a router"),
		//};
		if target_router==current_router
		{
			let target_server = target_server.expect("target server was not given.");
			for i in 0..topology.ports(current_router)
			{
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
					}
				}
			}
			unreachable!();
		}
		//let distance=topology.distance(current_router,target_router);
		//if distance==1
		let arrangement_size = topology.dragonfly_size().expect("This topology has not a dragonfly arrangement.");
		let (current_local,current_global)=arrangement_size.unpack(current_router);
		let (target_local,target_global)=arrangement_size.unpack(target_router);
		if current_global==target_global
		{
			// We are in the destination group. Use any local link.
			for i in 0..topology.ports(current_router)
			{
				if let (Location::RouterPort{router_index:other_router,..},_link_class)=topology.neighbour(current_router,i)
				{
					if other_router == target_router
					{
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
					}
				}
			}
			unreachable!();
		}
		if routing_info.hops == 0
		{
			let current_color = self.get_color(arrangement_size,current_local);
			let target_color = self.get_color(arrangement_size,target_local);
			// The first hop is usually a local one, but it could be a global one if the source is at the bridge.
			let middle_color = if current_color==target_color && current_global>target_global {1-current_color} else { current_color };
			let mut bridges = Vec::new();
			//let bridges = (0..arrangement_size.group_size).filter(|&bridge_local|
			for bridge_local in 0..arrangement_size.group_size
			{
				let bridge_color = self.get_color(arrangement_size,bridge_local);
				//println!("current={} bridge_local={} bridge_color={}",current_router,bridge_local,bridge_color);
				if bridge_color != middle_color { continue }
				let bridge = arrangement_size.pack( (bridge_local,current_global) );
				//let mut is_bridge = false;
				for i in 0..topology.ports(bridge)
				{
					if let (Location::RouterPort{router_index:other_router,..},_link_class)=topology.neighbour(bridge,i)
					{
						let (other_local,other_global)=arrangement_size.unpack(other_router);
						if other_global == target_global
						{
							let other_color = self.get_color(arrangement_size,other_local);
							assert!(other_color == bridge_color, "global link from {} to {} break the color",bridge,other_router);
							if bridge == current_router {
								// If we are in a bridge do not perform local link.
								// Or maybe we could want to perform a local for balancing considerations??
								return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
							} else {
								//is_bridge = true;
								bridges.push(bridge);
								break;
							}
						}
					}
				}
				//is_bridge
			}//).collect();
			assert!( !bridges.is_empty(), "No bridge found from current {} of color {} to target {} of color {} using middle color {}",current_router,current_color,target_router,target_color,middle_color);
			// Perform a local to any of the found bridges.
			let mut r = vec![];
			for i in 0..topology.ports(current_router)
			{
				if let (Location::RouterPort{router_index:other_router,..},_link_class)=topology.neighbour(current_router,i)
				{
					if bridges.contains(&other_router)
					{
						r.extend( (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)) );
					}
				}
			}
			assert!( !r.is_empty(), "no local links to bridges");
			return Ok(RoutingNextCandidates{candidates:r,idempotent:true});
		}
		// If we are not in the target group and we have given one hop then we have to advance a global link.
		for i in 0..topology.ports(current_router)
		{
			if let (Location::RouterPort{router_index:other_router,..},_link_class)=topology.neighbour(current_router,i)
			{
				let (_other_local,other_global)=arrangement_size.unpack(other_router);
				if other_global == target_global
				{
					return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
				}
			}
		}
		unreachable!()
	}
}

impl Dragonfly2ColorsRouting
{
	pub fn new(arg: RoutingBuilderArgument) -> Dragonfly2ColorsRouting
	{
		match_object_panic!(arg.cv,"Dragonfly2Colors",_value);
		Dragonfly2ColorsRouting{}
	}
	fn get_color(&self,size:ArrangementSize, switch_local:usize) -> u8
	{
		use std::convert::TryInto;
		// this works for palmtree. It would be better to have generally working code.
		let x = if switch_local*2 < size.group_size { switch_local } else { size.group_size-1 - switch_local };
		(x % 2).try_into().unwrap()
	}
}


#[cfg(test)]
mod tests {
	use super::*;
	use rand::SeedableRng;
	#[test]
	fn palmtree_valid()
	{
		let mut palmtree = Palmtree::default();
		let mut rng = StdRng::seed_from_u64(0);
		//let size = ArrangementSize { number_of_groups: 10, group_size: 5, number_of_ports: 3 };
		for (group_size,number_of_ports) in [(5,3), (8,4)]
		{
			let size = ArrangementSize { number_of_groups: group_size*number_of_ports+1, group_size, number_of_ports };
			palmtree.initialize(size,&mut rng);
			assert!( palmtree.is_valid(), "invalid arrangement {:?}", size );
			let gtdm = palmtree.global_trunking_distribution();
			assert!( *gtdm.outside_diagonal().min().unwrap() >0 , "some groups not connected {:?}",size);
		}
	}
	/// Checks whether the new definition matches the old one.
	#[test]
	fn extended_palmtree()
	{
		fn old_map( size:ArrangementSize, input:ArrangementPoint ) -> ArrangementPoint
		{
			let target_group_index = (
				input.group_index
				+ size.number_of_groups//to ensure being positive
				- (input.group_offset*size.number_of_ports+input.port_index+1)
			) % size.number_of_groups;
			let target_group_offset=(
				((size.number_of_groups+target_group_index-input.group_index)%size.number_of_groups) - 1
			) / size.number_of_ports;
			let target_port = size.number_of_ports-1-input.port_index;
			ArrangementPoint{
				group_index: target_group_index,
				group_offset: target_group_offset,
				port_index: target_port,
			}
		}
		for h in 1..10
		{
			let a = 2*h;
			let g = a*h+1;
			let size = ArrangementSize{ number_of_groups:g, group_size:a, number_of_ports:h};
			let palmtree = Palmtree{size};
			for input_group in 0..g
			{
				for input_offset in 0..a
				{
					for input_port in 0..h
					{
						let input = ArrangementPoint{group_index:input_group,group_offset:input_offset,port_index:input_port};
						let old_target = old_map(size,input);
						let target = palmtree.map(input);
						if target != old_target
						{
							panic!("The extended palmtree fails at {:?} for {:?}. old={:?} now={:?}",input,size,old_target,target);
						}
					}
				}
			}
		}
	}
}


/**
This is an adapted Valiant version for the Dragonfly topology, suitable for source adaptive routings, as UGAL.
It removes switches from source and target groups as intermediate switches.
```ignore
Valiant4Dragonfly{
	first: Shortest,
	second: Shortest,
	first_reserved_virtual_channels: [0],//optional parameter, defaults to empty. Reserves some VCs to be used only in the first stage
	second_reserved_virtual_channels: [1,2],//optional, defaults to empty. Reserves some VCs to be used only in the second stage.
	intermediate_bypass: CartesianTransform{sides:[4,4],project:[true,false]} //optional, defaults to None. A pattern on the routers such that when reaching a router `x` with `intermediate_bypass(x)==intermediate_bypass(Valiant4Dragonfly_choice)` the first stage is terminated.
	local_missrouting: true, //allow local missrouting, only if target in the group
	dragonfly_bypass: true, //Take shorcuts avoiding dumb hops
	legend_name: "Using Valiant4Dragonfly scheme, shortest to intermediate and shortest to destination",
}
```
 **/


#[derive(Debug)]
pub struct Valiant4Dragonfly
{
	first: Box<dyn Routing>,
	second: Box<dyn Routing>,
	//pattern to select intermideate nodes
	pattern:Box<dyn Pattern>,
	first_reserved_virtual_channels: Vec<usize>,
	second_reserved_virtual_channels: Vec<usize>,
	//exclude_h_groups:bool,
	intermediate_bypass: Option<Box<dyn Pattern>>,
	local_missrouting: bool, //only local missrouting if target in the group
dragonfly_bypass: bool, //lggl routes
}

impl Routing for Valiant4Dragonfly
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, num_virtual_channels:usize, rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		/*let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};*/
		let distance=topology.distance(current_router,target_router);
		if distance==0 //careful here
		{
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server.expect("There sould be a server here")
					{
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						//return (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
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
					let base=self.second.next(&meta[1].borrow(),topology,current_router,target_router, target_server,num_virtual_channels,rng)?;
					let idempotent = base.idempotent;


					let r=base.into_iter().filter_map(|egress|
						{
							if !self.first_reserved_virtual_channels.contains(&egress.virtual_channel)
							{
								Some(egress)

							}else{
								None
							}
						}).collect();

					Ok(RoutingNextCandidates{candidates:r,idempotent})
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
					let base = self.first.next(&meta[0].borrow(),topology,current_router, middle, Some(middle_server),  num_virtual_channels,rng)?;
					let idempotent = base.idempotent;
					let r=base.into_iter().filter_map(|mut egress|{
						//egress.hops = Some(routing_info.hops);
						if self.second_reserved_virtual_channels.contains(&egress.virtual_channel) { //may not be the best way....
							None
							/*if let Some(ref mut eh)=egress.estimated_remaining_hops
                            {
                                *eh += second_distance;
                            }
                            Some(egress)*/

						} else {
							if let Some(ref mut eh)=egress.estimated_remaining_hops
							{
								*eh += second_distance;
							}
							Some(egress)
						}
					}).collect();
					Ok(RoutingNextCandidates{candidates:r,idempotent})
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
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, _target_server:Option<usize>, rng: &mut StdRng)
	{
		let degree = topology.degree(current_router);
		let (server_source_location,_link_class) = topology.neighbour(current_router, degree);
		let source_server=match server_source_location
		{
			Location::ServerPort(server) =>server,
			_ => panic!("The server is not attached to a router"),
		};

		let cartesian_data = topology.cartesian_data().expect("Should be a cartesian data");//.expect("something").unpack(current_router)[1] ==

		let src_coord = cartesian_data.unpack(current_router);
		let trg_coord = cartesian_data.unpack(target_router);

		let mut middle = current_router;
		let mut middle_coord = cartesian_data.unpack(target_router);
		//degree is the degree of the topology

		if self.local_missrouting && (src_coord[1] == trg_coord[1])
		{
			//only local missrouting
			while src_coord[0] == middle_coord[0] || trg_coord[0] == middle_coord[0] {

				let middle_server = self.pattern.get_destination(source_server,topology, rng);
				let (middle_location,_link_class)=topology.server_neighbour(middle_server);
				middle=match middle_location
				{
					Location::RouterPort{router_index,router_port:_} =>router_index,
					_ => panic!("The server is not attached to a router"),
				};
				middle_coord = cartesian_data.unpack(middle);

			}
			middle_coord[1] = trg_coord[1];
			middle = cartesian_data.pack(&middle_coord);

		}else{ //general missrouting
			while src_coord[1] == middle_coord[1] || trg_coord[1] == middle_coord[1] {
				//||(self.exclude_h_groups && (((middle_coord[1] - (degree/2)*middle_coord[0]) % cartesian_data.sides[1])  > trg_coord[1]  &&  ((middle_coord[1] - (degree/2)*middle_coord[0] - (degree/2)) % cartesian_data.sides[1] ) <= trg_coord[1] ))  {

				//	middle = rng.gen_range(0..topology.num_routers());

				let middle_server = self.pattern.get_destination(source_server,topology, rng);
				let (middle_location,_link_class)=topology.server_neighbour(middle_server);
				middle=match middle_location
				{
					Location::RouterPort{router_index,router_port:_} =>router_index,
					_ => panic!("The server is not attached to a router"),
				};
				middle_coord = cartesian_data.unpack(middle);

			}
		}

		// if middle == current_router && !self.use_min_b{
		// 	middle = target_router;
		// }

		let mut bri=routing_info.borrow_mut();
		bri.visited_routers=Some(vec![current_router]);
		bri.meta=Some(vec![RefCell::new(RoutingInfo::new()),RefCell::new(RoutingInfo::new())]);

		bri.selections=Some(vec![middle as i32]);
		self.first.initialize_routing_info(&bri.meta.as_ref().unwrap()[0],topology,current_router,middle,None,rng)
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
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
					self.second.update_routing_info(&meta[1],topology,current_router,current_port,target_router,target_server,rng);
				}
			Some(middle) =>
				{
					let at_middle = if let Some(ref pattern) = self.intermediate_bypass {
						let proj_middle = pattern.get_destination(middle, topology, rng);
						let proj_current = pattern.get_destination(current_router, topology, rng);
						proj_middle == proj_current
					}else if self.dragonfly_bypass && //this is a hack which looks ugly but works
						topology.cartesian_data().expect("something").unpack(current_router)[1] ==
							topology.cartesian_data().expect("something").unpack(middle)[1] //were on the same group
						&& topology.distance(middle, target_router) < topology.distance(current_router, target_router)
					{
						true
					}else{
						current_router == middle
					};
					if at_middle
					{
						bri.selections=None;
						let meta=bri.meta.as_ref().unwrap();
						self.second.initialize_routing_info(&meta[1],topology,current_router,target_router,target_server,rng);
					}
					else
					{
						let meta=bri.meta.as_mut().unwrap();
						meta[0].borrow_mut().hops+=1;
						self.first.update_routing_info(&meta[0],topology,current_router,current_port,middle,None,rng);
					}
				}
		};


		match bri.visited_routers
		{
			Some(ref mut v) =>
				{
					v.push(current_router);
				}
			None => panic!("visited_routers not initialized"),
		};


	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.first.initialize(topology,rng);
		self.second.initialize(topology,rng);
		self.pattern.initialize(topology.num_servers(), topology.num_servers(), topology, rng);
		if let Some(ref mut pattern) = self.intermediate_bypass
		{
			let size = topology.num_routers();
			pattern.initialize(size,size,topology,rng);
		}
	}
	fn performed_request(&self, requested:&CandidateEgress, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, num_virtual_channels:usize, rng:&mut StdRng)
	{
		let mut bri=routing_info.borrow_mut();
		let middle = bri.selections.as_ref().map(|s| s[0] as usize);
		let meta=bri.meta.as_mut().unwrap();

		match middle
		{
			None =>
				{
					//Already towards true destination
					self.first.performed_request(requested,&meta[1],topology,current_router,target_router,target_server,num_virtual_channels,rng);
				}
			Some(_) =>
				{
					//Already towards true destination
					self.second.performed_request(requested,&meta[0],topology,current_router,target_router,target_server,num_virtual_channels,rng);
				}
		};
	}
}

impl Valiant4Dragonfly
{
	pub fn new(arg: RoutingBuilderArgument) -> Valiant4Dragonfly
	{
		//let mut order=None;
		//let mut servers_per_router=None;
		let mut first=None;
		let mut second=None;
		let mut pattern: Box<dyn Pattern> = Box::new(UniformPattern::uniform_pattern(true)); //pattern to intermideate node
		// let mut exclude_h_groups=false;
		let mut first_reserved_virtual_channels=vec![];
		let mut second_reserved_virtual_channels=vec![];
		let mut local_missrouting=false;
		let mut intermediate_bypass=None;
		let mut dragonfly_bypass=false;
		match_object_panic!(arg.cv,"Valiant4Dragonfly",value,
			"first" => first=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"second" => second=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"pattern" => pattern= Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})).expect("pattern not valid for Valiant4Dragonfly"),
			// "exclude_h_groups"=> exclude_h_groups=value.as_bool().expect("bad value for exclude_h_groups"),
			"first_reserved_virtual_channels" => first_reserved_virtual_channels=value.
				as_array().expect("bad value for first_reserved_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in first_reserved_virtual_channels") as usize).collect(),
			"second_reserved_virtual_channels" => second_reserved_virtual_channels=value.
				as_array().expect("bad value for second_reserved_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in second_reserved_virtual_channels") as usize).collect(),
			"intermediate_bypass" => intermediate_bypass=Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"local_missrouting" => local_missrouting=value.as_bool().expect("bad value for local_missrouting"),
			"dragonfly_bypass" => dragonfly_bypass=value.as_bool().expect("bad value for dragonfly_bypass"),
		);
		let first=first.expect("There were no first");
		let second=second.expect("There were no second");

		Valiant4Dragonfly{
			first,
			second,
			pattern,
			first_reserved_virtual_channels,
			second_reserved_virtual_channels,
			intermediate_bypass,
			local_missrouting,
			dragonfly_bypass,
		}
	}
}






