

use quantifiable_derive::Quantifiable;//the derive macro
use super::prelude::*;

/**
Implementation of the so called Megafly or Dragonfly+ interconnection network topology.
It is an indirect network with two levels of switches. The switches in the first level can be called leaf switches and are the switches with servers attached. The switches in the second level can be called spine and are connected to both first-level switches and to other second-level switches.
The network is divided in blocks. inside each block the connection between the first level and the second level switches follows a fat-tree connectivity, this is, a complete-bipartite graph. The links between second level switches connect switches of different block, with exactly one such link between each pair of block.

REFS Megafly, Dragonfly+

Shpiner, A., Haramaty, Z., Eliad, S., Zdornov, V., Gafni, B., & Zahavi, E. (2017, February). Dragonfly+: Low cost topology for scaling datacenters. In 2017 IEEE 3rd International Workshop on High-Performance Interconnection Networks in the Exascale and Big-Data Era (HiPINEB) (pp. 1-8). IEEE.

Flajslik, M., Borch, E., & Parker, M. A. (2018). Megafly: A Topology for Exascale Systems. High Performance Computing, 289–310. doi:10.1007/978-3-319-92040-5_15

Chen, D., Heidelberger, P., Stunkel, C., Sugawara, Y., Minkenberg, C., Prisacari,
B., Rodriguez, G.: An evaluation of network architectures for next generation
supercomputers. In: 2016 7th International Workshop on Performance Modeling,
Benchmarking and Simulation of High Performance Computer Systems (PMBS),
pp. 11–21, November 2016

Mellanox Technologies, “Interconnect your future with Mellanox,”
http://www.slideshare.net/mellanox/140307-mellanox-versus-cray-p,
accessed: 2016-04-10.
https://documents.pub/document/interconnect-your-future-with-mellanox.html?page=22 says 06-may-2015

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
}

impl Topology for Megafly
{
	fn num_routers(&self) -> usize
	{
		todo!()
	}
	fn num_servers(&self) -> usize
	{
		todo!()
	}
	fn neighbour(&self, router_index:usize, port: usize) -> (Location,usize)
	{
		todo!()
	}
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		todo!()
	}
	fn diameter(&self) -> usize
	{
		todo!()
	}
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		todo!()
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
		todo!()
	}
	fn minimum_degree(&self) -> usize
	{
		todo!()
	}
	fn degree(&self, _router_index: usize) -> usize
	{
		todo!()
	}
	fn ports(&self, _router_index: usize) -> usize
	{
		todo!()
	}
	fn cartesian_data(&self) -> Option<&CartesianData>
	{
		todo!()
	}
	fn coordinated_routing_record(&self, _coordinates_a:&[usize], _coordinates_b:&[usize], _rng: Option<&RefCell<StdRng>>)->Vec<i32>
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


