
use std::cell::RefCell;
use ::rand::{rngs::StdRng};
use super::{Topology,Location};
use super::cartesian::CartesianData;
use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::matrix::Matrix;
use crate::quantify::Quantifiable;
use crate::match_object_panic;

///Builds a dragonfly topology with canonic dimensions and palm-tree arrangement of global links.
///The canonic dimensions means
///* to have as many global links as links to servers in each router,
///* to have in each group the double number of routers than links to a server in a router,
///* to have a unique global link joining each pair of groups,
///* and to have a unique local link joining each pair of router in the same group.
///For the palm-tree arrangement we refer to the doctoral thesis of Marina García.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CanonicDragonfly
{
	/// Number of ports per router that connect to routers in a different group. Dally called it `h`
	global_ports_per_router: usize,
	/// Number of servers per router. Dally called it `p`. Typically p=h.
	servers_per_router: usize,
	/// Configuration of the global links.
	global_arrangement: Box<dyn Arrangement>,

	// cached values:

	/// Number of routers in a group. Dally called it `a`. a-1 local ports. In a canonic dragonfly a=2h.
	group_size: usize,
	/// Number of groups = a*h+1. Dally called it `g`.
	number_of_groups: usize,
	/// `distance_matrix.get(i,j)` = distance from router i to router j.
	distance_matrix:Matrix<u8>,
}

impl Topology for CanonicDragonfly
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
			let size = ArrangementSize{
				number_of_groups: self.number_of_groups,
				group_size: self.group_size,
				number_of_ports: self.global_ports_per_router,
			};
			assert!(size.contains(point), "arrangement point {:?} is not in range. size is {:?}",point,size);
			let target_point = self.global_arrangement.map(size,point);
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
		None
	}
	fn coordinated_routing_record(&self, _coordinates_a:&[usize], _coordinates_b:&[usize], _rng: Option<&RefCell<StdRng>>)->Vec<i32>
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
}

impl CanonicDragonfly
{
	pub fn new(cv:&ConfigurationValue) -> CanonicDragonfly
	{
		let mut global_ports_per_router=None;
		let mut servers_per_router=None;
		match_object_panic!(cv,"CanonicDragonfly",value,
			"global_ports_per_router" => global_ports_per_router=Some(value.as_f64().expect("bad value for global_ports_per_router")as usize),
			"servers_per_router" => servers_per_router=Some(value.as_f64().expect("bad value for servers_per_router")as usize),
		);
		let global_ports_per_router=global_ports_per_router.expect("There were no global_ports_per_router");
		let servers_per_router=servers_per_router.expect("There were no servers_per_router");
		let group_size = 2*global_ports_per_router;
		let number_of_groups = group_size*global_ports_per_router + 1;
		let mut topo=CanonicDragonfly{
			global_ports_per_router,
			servers_per_router,
			global_arrangement: Box::new(Palmtree),
			group_size,
			number_of_groups,
			distance_matrix:Matrix::constant(0,0,0),
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

#[derive(Clone,Copy,Debug)]
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
}

pub trait Arrangement : Quantifiable + core::fmt::Debug
{
	fn map( &self, size:ArrangementSize, input:ArrangementPoint ) -> ArrangementPoint;
	/// Checks whether the arrangement is involution and in range
	fn is_valid( &self, size:ArrangementSize ) -> bool
	{
		for group_index in 0..size.number_of_groups
		{
			for group_offset in 0..size.group_size
			{
				for port_index in 0..size.number_of_ports
				{
					let input = ArrangementPoint{group_index,group_offset,port_index};
					let target = self.map(size,input);
					if !size.contains(target) { return false }//has to be in range
					let back = self.map(size,target);
					if input != back { return false }//has to be an involution
				}
			}
		}
		true
	}
	/// For each pair of groups count the number of links.
	fn global_trunking_distribution( &self, size:ArrangementSize ) -> Matrix<usize>
	{
		let mut result : Matrix<usize> = Matrix::constant(0,size.number_of_groups,size.number_of_groups);
		for group_index in 0..size.number_of_groups
		{
			for group_offset in 0..size.group_size
			{
				for port_index in 0..size.number_of_ports
				{
					let input = ArrangementPoint{group_index,group_offset,port_index};
					let target = self.map(size,input);
					*result.get_mut(input.group_index,target.group_index)+=1;
				}
			}
		}
		return result;
	}
}

/// Marina García's regular arrangement for the dragonfly.
/// Only works for `number_of_groups=group_size*number_of_ports+1`.
#[derive(Quantifiable,Debug)]
pub struct Palmtree;

impl Arrangement for Palmtree
{
	fn map( &self, size:ArrangementSize, input:ArrangementPoint ) -> ArrangementPoint
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
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn palmtree_valid()
	{
		let palmtree = Palmtree;
		//let size = ArrangementSize { number_of_groups: 10, group_size: 5, number_of_ports: 3 };
		for (group_size,number_of_ports) in [(5,3), (8,4)]
		{
			let size = ArrangementSize { number_of_groups: 16, group_size: 5, number_of_ports: 3 };
			assert!( palmtree.is_valid(size), "invalid arrangement {:?}", size );
			let gtdm = palmtree.global_trunking_distribution(size);
			assert!( *gtdm.outside_diagonal().min().unwrap() >0 , "some groups not connected {:?}",size);
		}
	}
}

