

use quantifiable_derive::Quantifiable;//the derive macro
use super::prelude::*;
use crate::matrix::Matrix;
use super::dragonfly::{Arrangement,ArrangementPoint,ArrangementSize,Palmtree,new_arrangement};
use crate::config_parser::ConfigurationValue;
use crate::match_object_panic;

/**
Implementation of the so called Megafly or Dragonfly+ interconnection network topology.
It is an indirect network with two levels of switches. The switches in the first level can be called leaf switches and are the switches with servers attached. The switches in the second level can be called spine and are connected to both first-level switches and to other second-level switches.
The network is divided in blocks. inside each block the connection between the first level and the second level switches follows a fat-tree connectivity, this is, a complete-bipartite graph. The links between second level switches connect switches of different block, with exactly one such link between each pair of block.


The routes suggested by Shpiner et al. are
+ High priority L-G-L.
+ Medium priority L-G-G-L.
+ Low priority L-G-L-L-G-L. Like Valiant, but not necessarily decided at source.

REFS Megafly, Dragonfly+


Mellanox Technologies, “Interconnect your future with Mellanox,” March 2014
<http://www.slideshare.net/mellanox/140307-mellanox-versus-cray-p>,
accessed: 2016-04-10.
<https://documents.pub/document/interconnect-your-future-with-mellanox.html?page=22> says 06-may-2015

Chen, D., Heidelberger, P., Stunkel, C., Sugawara, Y., Minkenberg, C., Prisacari, B., Rodriguez, G.: An evaluation of network architectures for next generation supercomputers. In: 2016 7th International Workshop on Performance Modeling, Benchmarking and Simulation of High Performance Computer Systems (PMBS), pp. 11–21, November 2016

Shpiner, A., Haramaty, Z., Eliad, S., Zdornov, V., Gafni, B., & Zahavi, E. (2017, February). Dragonfly+: Low cost topology for scaling datacenters. In 2017 IEEE 3rd International Workshop on High-Performance Interconnection Networks in the Exascale and Big-Data Era (HiPINEB) (pp. 1-8). IEEE.

Flajslik, M., Borch, E., & Parker, M. A. (2018). Megafly: A Topology for Exascale Systems. High Performance Computing, 289–310. doi:10.1007/978-3-319-92040-5_15


**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Megafly
{
	/// Number of ports per leaf switch that connect to servers.
	servers_per_leaf: usize,
	/// Number of leaf switches in a block/group. Each block has also this same number of spine switches.
	group_size: usize,
	/// Number of ports per spine switch that connect to switches in a different group.
	global_ports_per_spine: usize,
	/// Number of groups in the whole network. Each group with `group_size` leaf switches and spine switches.
	number_of_groups: usize,
	/// Configuration of the global links.
	global_arrangement: Box<dyn Arrangement>,
	///`distance_matrix.get(i,j)` = distance from router i to router j.
	distance_matrix:Matrix<u8>,
}

impl Topology for Megafly
{
	fn num_routers(&self) -> usize
	{
		// Both leaf and spine switches.
		self.number_of_groups * self.group_size * 2
	}
	fn num_servers(&self) -> usize
	{
		self.number_of_groups * self.group_size * self.servers_per_leaf
	}
	fn neighbour(&self, router_index:usize, port: usize) -> (Location,usize)
	{
		// link class 0 : local link. Following complete bipartite connectivity inside the group.
		// link class 1 : global link. Connects from a spine to other groups. Follows some pattern to be determined.
		// link class 2 : server to leaf switch.
		// switches are numbered by levels. First all leaf switches and then all spine switches.
		// Among each level, switches are numbered first by their group then by their position in the group.
		// This is, lexicographic numbering for coordinates `(level,group_index,group_offset)`.
		// Note that the pack/unpack functions have the order reversed, in a typical little endian way.
		// 0 <= level <= 1
		// 0 <= group_index < number_of_groups
		// 0 <= group_offset < group_size
		let (router_local,router_global,level_index)=self.unpack(router_index);
		//println!("router_index={router_index} router_local={router_local} router_global={router_global} level={level_index}");
		match level_index
		{
			0 => {
				if port < self.group_size
				{
					// Upwards link
					// just swap the local position with the port.
					(Location::RouterPort{router_index:self.pack((port,router_global,1)),router_port:router_local},0)
				}
				else
				{
					// Link to servers
					let port_offset = port - self.group_size;
					(Location::ServerPort(router_index*self.servers_per_leaf + port_offset),2)
				}
			},
			1 => {
				if port < self.group_size
				{
					// Downwards link
					// just swap the local position with the port.
					(Location::RouterPort{router_index:self.pack((port,router_global,0)),router_port:router_local},0)
				}
				else
				{
					// Global link
					let port_offset = port - self.group_size;
					let point = ArrangementPoint {
						group_index: router_global,
						group_offset: router_local,
						port_index: port_offset,
					};
					//assert!(size.contains(point), "arrangement point {:?} is not in range. size is {:?}",point,size);
					let target_point = self.global_arrangement.map(point);
					let target_global = target_point.group_index;
					let target_local = target_point.group_offset;
					let target_port = self.group_size + target_point.port_index;
					//println!("{},{} g{} -> {},{} g{}",router_local,router_global,port_offset,target_local,target_global,target_port+1-self.group_size);
					(Location::RouterPort{router_index:self.pack((target_local,target_global,1)),router_port:target_port},1)
				}
			},
			_ => unreachable!("level must be 0 or 1"),
		}
	}
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let degree = self.group_size;//ports before servers.
		(Location::RouterPort{
			router_index: server_index/self.servers_per_leaf,
			router_port: degree + server_index%self.servers_per_leaf,
		},2)
	}
	fn diameter(&self) -> usize
	{
		// Note that the distance between switches could be as high as 5.
		3
	}
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		(*self.distance_matrix.get(origin,destination)).into()
	}
	fn amount_shortest_paths(&self,_origin:usize,_destination:usize) -> usize
	{
		todo!()
	}
	fn average_amount_shortest_paths(&self) -> f32
	{
		todo!()
	}
	fn maximum_degree(&self) -> usize
	{
		self.group_size + self.global_ports_per_spine
	}
	fn minimum_degree(&self) -> usize
	{
		self.group_size
	}
	fn degree(&self, router_index: usize) -> usize
	{
		let (_router_local,_router_global,level_index)=self.unpack(router_index);
		match level_index
		{
			0 => self.group_size,
			1 => self.group_size + self.global_ports_per_spine,
			_ => unreachable!(),
		}
	}
	fn ports(&self, router_index: usize) -> usize
	{
		let (_router_local,_router_global,level_index)=self.unpack(router_index);
		match level_index
		{
			0 => self.group_size + self.servers_per_leaf,
			1 => self.group_size + self.global_ports_per_spine,
			_ => unreachable!(),
		}
	}
	fn cartesian_data(&self) -> Option<&CartesianData>
	{
		todo!()
	}
	fn coordinated_routing_record(&self, _coordinates_a:&[usize], _coordinates_b:&[usize], _rng: Option<&mut StdRng>)->Vec<i32>
	{
		todo!()
	}
	fn is_direction_change(&self, _router_index:usize, _input_port: usize, _output_port: usize) -> bool
	{
		todo!()
	}
	fn up_down_distance(&self,_origin:usize,_destination:usize) -> Option<(usize,usize)>
	{
		todo!()
	}
}


impl Megafly
{
	pub fn new(arg:TopologyBuilderArgument) -> Megafly
	{
		let mut global_ports_per_spine=None;
		let mut servers_per_leaf=None;
		let mut group_size=None;
		let mut number_of_groups=None;
		let mut global_arrangement=None;
		match_object_panic!(arg.cv,"Megafly",value,
			"global_ports_per_spine" => global_ports_per_spine=Some(value.as_f64().expect("bad value for global_ports_per_spine")as usize),
			"servers_per_leaf" => servers_per_leaf=Some(value.as_f64().expect("bad value for servers_per_leaf")as usize),
			"group_size" => group_size=Some(value.as_f64().expect("bad value for group_size")as usize),
			"number_of_groups" => number_of_groups=Some(value.as_f64().expect("bad value for number_of_groups")as usize),
			"global_arrangement" => global_arrangement=Some(new_arrangement(value.into())),
		);
		let global_ports_per_spine=global_ports_per_spine.expect("There were no global_ports_per_spine");
		let servers_per_leaf=servers_per_leaf.expect("There were no servers_per_leaf");
		let group_size=group_size.expect("There were no group_size");
		let number_of_groups=number_of_groups.expect("There were no number_of_groups");
		let mut global_arrangement = global_arrangement.unwrap_or_else(||Box::new(Palmtree::default()));
		global_arrangement.initialize(ArrangementSize{
			number_of_groups,
			group_size,
			number_of_ports: global_ports_per_spine,
		},arg.rng);
		//let group_size = 2*global_ports_per_spine;
		//let number_of_groups = group_size*global_ports_per_spine + 1;
		let mut topo=Megafly{
			global_ports_per_spine,
			servers_per_leaf,
			global_arrangement,
			group_size,
			number_of_groups,
			distance_matrix:Matrix::constant(0,0,0),
		};
		let (distance_matrix,_amount_matrix)=topo.compute_amount_shortest_paths();
		topo.distance_matrix=distance_matrix.map(|x|*x as u8);
		topo
	}
	/**
	 Unpack a switch index into `(group_offset, group_index, level)` coordinates.
	 With `group_offset` beings the position of the switch in the group and `group_index` the index of the group.
	 `level=0` for leaf switches and `level=1` for spine switches.
	**/
	fn unpack(&self, router_index: usize) -> (usize,usize,usize)
	{
		let size = self.number_of_groups * self.group_size;
		let level_index = router_index / size;
		let level_offset = router_index % size;
		let group_index = level_offset / self.group_size;
		let group_offset = level_offset % self.group_size;
		( group_offset, group_index, level_index )
	}
	/**
	 Pack coordinates `(group_offset, group_index, level)` into a whole switch index.
	**/
	fn pack(&self, coordinates:(usize,usize,usize)) -> usize
	{
		let (group_offset, group_index, level_index) = coordinates;
		let level_offset = group_offset + group_index*self.group_size;
		level_offset + level_index*self.group_size*self.number_of_groups
	}
}

