
use std::cell::RefCell;
use ::rand::{rngs::StdRng};
use super::{Topology,Location};
use super::cartesian::CartesianData;
use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::matrix::Matrix;

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
	/// Configuration of the global links. XXX XXX XXX

	// cached values:

	/// Number of routers in a group. Dally called it `a`. a-1 local ports. In a canonic dragonfly a=2h.
	group_size: usize,
	/// Number of groups = a*h+1. Dally called it `g`.
	number_of_groups: usize,
	///distance_matrix.get(i,j) = distance from router i to router j
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
			let port_offset=port+1-self.group_size;
			let target_global=(router_global+self.number_of_groups-(router_local*self.global_ports_per_router+port_offset+1)) % self.number_of_groups;
			let target_local=( ((self.number_of_groups+target_global-router_global)%self.number_of_groups)-1 )/self.global_ports_per_router;
			let target_port=self.group_size-1  +  self.global_ports_per_router-1-port_offset;
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
		let r=self.group_size-1 + self.global_ports_per_router;
		(Location::RouterPort{
			router_index: server_index/self.servers_per_router,
			router_port: r+server_index%self.servers_per_router,
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
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=cv
		{
			if cv_name!="CanonicDragonfly"
			{
				panic!("A CanonicDragonfly must be created from a `CanonicDragonfly` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"global_ports_per_router" => match value
					{
						&ConfigurationValue::Number(f) => global_ports_per_router=Some(f as usize),
						_ => panic!("bad value for global_ports_per_router"),
					}
					"servers_per_router" => match value
					{
						&ConfigurationValue::Number(f) => servers_per_router=Some(f as usize),
						_ => panic!("bad value for servers_per_router"),
					}
					_ => panic!("Nothing to do with field {} in CanonicDragonfly",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a CanonicDragonfly from a non-Object");
		}
		let global_ports_per_router=global_ports_per_router.expect("There were no global_ports_per_router");
		let servers_per_router=servers_per_router.expect("There were no servers_per_router");
		let group_size = 2*global_ports_per_router;
		let number_of_groups = group_size*global_ports_per_router + 1;
		let mut topo=CanonicDragonfly{
			global_ports_per_router,
			servers_per_router,
			group_size,
			number_of_groups,
			distance_matrix:Matrix::constant(0,0,0),
		};
		let (distance_matrix,_amount_matrix)=topo.compute_amount_shortest_paths();
		topo.distance_matrix=distance_matrix.map(|x|*x as u8);
		topo
	}
	fn unpack(&self, router_index: usize) -> (usize,usize)
	{
		(router_index%self.group_size,router_index/self.group_size)
	}
	fn pack(&self, coordinates:(usize,usize)) -> usize
	{
		coordinates.0+coordinates.1*self.group_size
	}
}


