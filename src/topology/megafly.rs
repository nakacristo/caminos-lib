

use crate::PatternBuilderArgument;
use crate::pattern::new_optional_pattern;
use quantifiable_derive::Quantifiable;//the derive macro
use super::prelude::*;
use crate::matrix::Matrix;
use super::dragonfly::{Arrangement,ArrangementPoint,ArrangementSize,Palmtree,new_arrangement};
use crate::config_parser::ConfigurationValue;
use crate::match_object_panic;
use crate::pattern::Pattern;
use crate::routing::{CandidateEgress, Error, Routing, RoutingBuilderArgument, RoutingInfo, RoutingNextCandidates};
use crate::topology::NeighbourRouterIteratorItem;

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
	// Cartesian data [switch_index, group_index]
	cartesian_data: CartesianData,
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
		Some(&self.cartesian_data)
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
		let mut lag=1;
		match_object_panic!(arg.cv,"Megafly",value,
			"global_ports_per_spine" => global_ports_per_spine=Some(value.as_f64().expect("bad value for global_ports_per_spine")as usize),
			"servers_per_leaf" => servers_per_leaf=Some(value.as_f64().expect("bad value for servers_per_leaf")as usize),
			"group_size" => group_size=Some(value.as_f64().expect("bad value for group_size")as usize),
			"number_of_groups" => number_of_groups=Some(value.as_f64().expect("bad value for number_of_groups")as usize),
			"global_arrangement" => global_arrangement=Some(new_arrangement(value.into())),
			"lag" | "global_lag" => lag=value.as_usize().expect("bad value for lag"),
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
			lag,
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
			cartesian_data: CartesianData::new(&[group_size, number_of_groups]),
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

pub fn pack_source_destination_server(source_server: usize, destination_server:usize, total_servers:usize) -> usize
{
	source_server*total_servers + destination_server
}

/**
MegaflyAD routing from Indirect adaptive routing...
It uses 2VCs, one for the 1st segment, and another for the 2nd segment.
```ignore
MegaflyAD{

}
```
 **/
#[derive(Debug)]
pub struct MegaflyAD
{
	first_allowed_virtual_channels: Vec<usize>,
	second_allowed_virtual_channels: Vec<usize>,
	minimal_to_deroute: Vec<usize>,
	source_group_pattern:  Vec<Vec<Option<Box<dyn Pattern>>>>,
	intermediate_group_pattern:  Vec<Vec<Option<Box<dyn Pattern>>>>,
	destination_group_pattern:  Vec<Vec<Option<Box<dyn Pattern>>>>,
	global_pattern_per_hop: Vec<Vec<Option<Box<dyn Pattern>>>>,
	consume_same_channel: bool,
	set_global_minimal_channel: bool,
}

impl Routing for MegaflyAD
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, num_virtual_channels:usize, _rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
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
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true});
					}
				}
			}
			unreachable!();
		}

		// let binding = routing_info.auxiliar.borrow();
		// let aux = binding.as_ref().unwrap().downcast_ref::<Vec<usize>>().unwrap();
		// let vc_local = aux[0] + aux[1] * 2;
		// let vc_global = aux[1];
		// let _meta=routing_info.meta.as_ref().unwrap();
		let visited_routers = routing_info.visited_routers.as_ref().unwrap();
		let source_router = visited_routers[0];
		let selections = routing_info.selections.as_ref().unwrap();

		let cartesian_data = topology.cartesian_data().expect("cartesian data not available"); //BEAWARE THAT DF+ IS AN INDIRECT NETWORK
		let current_coord = cartesian_data.unpack(current_router);
		let target_coord = cartesian_data.unpack(target_router);
		let source_coord = cartesian_data.unpack(source_router);

		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);

		let pos_source_group = source_coord[1] == current_coord[1];
		let pos_target_group = target_coord[1] == current_coord[1];
		let pos_intermediate_group = source_coord[1] != current_coord[1] && current_coord[1] != target_coord[1];

		let source_server = routing_info.source_server.unwrap();
		let target_server = target_server.unwrap();
		let index_pair = pack_source_destination_server(source_server, target_server, topology.num_servers());

		for NeighbourRouterIteratorItem{link_class: next_link_class,port_index,neighbour_router:neighbour_router_index,..} in topology.neighbour_router_iter(current_router)
		{
			let neighbour_coord = cartesian_data.unpack(neighbour_router_index);
			let next_distance = topology.distance(neighbour_router_index, target_router);

			let minimal = if next_distance < distance
			{
				0 //salto minimo
			}else {
				1 //salto no minimo
			};

			match next_link_class {
				0 =>{ //local link
					match selections[0]
					{
						0 =>{ //up

							if self.minimal_to_deroute[0] == 0 && minimal == 1{
								continue;
							}

							// if pos_target_group {
							// 		if let Some(ref pattern) = self.source_group_pattern[0][0]
							// 		{
							// 			let port = if let Some(ref pattern) = self.source_group_pattern[0][1]
							// 			{
							// 				pattern.get_destination(port_index, topology, _rng)
							// 			} else {
							// 				port_index
							// 			};
							//
							// 			let destination = pattern.get_destination(index_pair, topology, _rng);
							// 			if destination != port {
							// 				continue;
							// 			}
							// 		}
							// }


							// if minimal == 1
							// {
							// 	if let Some(ref pattern) = self.local_pattern_per_hop[0][0]
							// 	{
							// 		let port = if let Some(ref pattern) = self.local_pattern_per_hop[0][1]
							// 		{
							// 			pattern.get_destination(port_index, topology, _rng)
							// 		} else {
							// 			port_index
							// 		};
							//
							// 		let destination = pattern.get_destination(index_pair, topology, _rng);
							// 		if destination != port {
							// 			continue;
							// 		}
							// 	}
							// }

							r.extend(self.first_allowed_virtual_channels.iter().map(|v| CandidateEgress{port:port_index,virtual_channel:*v,label:minimal,..Default::default()}));
						}
						1 => { //down

							if pos_target_group{

								if minimal == 1 //non-minimal
								{
									if  selections[1] == 0 || selections[1] == 2 || self.minimal_to_deroute[2] == 0{
										continue;
									}

									if let Some(ref pattern) = self.destination_group_pattern[1][0]
									{
										let destination = pattern.get_destination(index_pair ,topology,_rng);

										let neighbour_hash = if let Some(ref pattern) = self.destination_group_pattern[1][1]
										{
											pattern.get_destination(neighbour_coord[0], topology, _rng)
										} else {
											neighbour_coord[0]
										};

										if destination != neighbour_hash{
											continue;
										}
									}

								}

							}else if pos_source_group{

								continue;

							} else { //were in the intermediate group

								if distance == 2{ //si estas a gd no bajar.
									continue;
								}

								if let Some(ref pattern) = self.intermediate_group_pattern[1][0]
								{
									let destination = pattern.get_destination(index_pair ,topology,_rng);

									let neighbour_hash = if let Some(ref pattern) = self.intermediate_group_pattern[1][1]
									{
										pattern.get_destination(neighbour_coord[0], topology, _rng)
									} else {
										neighbour_coord[0]
									};

									if destination != neighbour_hash{
										continue;
									}
								}
							}

							if selections[1] < 2 && ((!pos_target_group && self.consume_same_channel) || !self.consume_same_channel) { //FIXME: FOR MISSROUTE IN DEST GROUP

								r.extend(self.first_allowed_virtual_channels.iter().map(|v| CandidateEgress{port:port_index,virtual_channel:*v,label:minimal,..Default::default()}));

							}else{

								r.extend(self.second_allowed_virtual_channels.iter().map(|v| CandidateEgress{port:port_index,virtual_channel:*v,label:minimal,..Default::default()}));

							}

						}
						2 =>{ //up
							if minimal == 0{
								r.extend(self.second_allowed_virtual_channels.iter().map(|v| CandidateEgress::new(port_index, *v)));
							}
						}
						3 =>{ //down
							if next_distance == 0{
								r.extend(self.second_allowed_virtual_channels.iter().map(|v| CandidateEgress::new(port_index, *v)));
							}
						}
						_ => {
							panic!("Megafly route through more than 4 local links")
						}
					}

				}
				1 =>{ //global link
					let real_minimal = if neighbour_coord[1] == target_coord[1] {0} else {1};
					match selections[1]
					{
						0 =>{
							if pos_target_group{

								continue;

							} else {
								if self.minimal_to_deroute[1] == 0 && real_minimal == 1{
									continue;
								}
								if real_minimal == 1
								{
									if let Some(ref pattern) = self.global_pattern_per_hop[0][0]
									{
										let port = if let Some(ref pattern) = self.global_pattern_per_hop[0][1]
										{
											pattern.get_destination(neighbour_coord[1], topology, _rng)
										} else {
											neighbour_coord[1]
										};

										let destination = pattern.get_destination(index_pair, topology, _rng);
										if destination != port {
											continue;
										}
									}
								}

								if real_minimal == 0 && self.set_global_minimal_channel
								{
									r.extend(self.second_allowed_virtual_channels.iter().map(|v| CandidateEgress { port: port_index, virtual_channel: *v, label: real_minimal, ..Default::default() }));

								}else{
									r.extend(self.first_allowed_virtual_channels.iter().map(|v| CandidateEgress { port: port_index, virtual_channel: *v, label: real_minimal, ..Default::default() }));

								}
							}
						}

						1 =>{
							if pos_intermediate_group{

								if real_minimal == 0 {
									r.extend(self.second_allowed_virtual_channels.iter().map(|v| CandidateEgress::new(port_index, *v)));
								}

							} else {
								continue;
							}
						}
						_ => {
							continue;
							// panic!("Megafly route through more than 2 global links")
						}
					}
				}
				_ => panic!("Megafly only route through local and global links"),
			}
		}

		//if r is 0 panic
		if r.len() == 0
		{
			//print some info
			println!("current_router={}",current_router);
			println!("target_router={}",target_router);
			println!("selections={:?}",selections);
			panic!("MegaflyAD routing found no candidates");
		}
		//FIXME: we can recover idempotence in some cases.
		Ok(RoutingNextCandidates{candidates:r, idempotent:false})
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, current_router:usize, _target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		let link_usage= vec![0, 0];
		let mut bri=routing_info.borrow_mut();
		bri.visited_routers=Some(vec![current_router]);
		//bri.meta=Some(vec![RefCell::new(RoutingInfo::new()),RefCell::new(RoutingInfo::new())]);
		// bri.meta=Some(vec![RefCell::new(RoutingInfo::new()),RefCell::new(RoutingInfo::new())]);
		// self.first.initialize_routing_info(&bri.meta.as_ref().unwrap()[0],topology,current_router,target_router,None,rng);
		// self.second.initialize_routing_info(&bri.meta.as_ref().unwrap()[1],topology,current_router,target_router,None,rng);
		 bri.selections=Some(link_usage);
		// bri.auxiliar= RefCell::new(Some(Box::new(vec![0usize, 0usize])));
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, _target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		let (_router_location,link_class) = topology.neighbour(current_router, current_port);
		//if self.link_restrictions.contains(&link_class) { cs = vec![cs[0]]; };
		let mut bri=routing_info.borrow_mut();

		// let routing_index; //if bri.selections.as_ref().unwrap()[0] == 0 { &self.first } else { &self.second };

		let cs = match bri.selections
		{
			None => unreachable!(),
			Some(ref x) =>
			{
				if link_class == 0
				{
					vec![x[0]+1, x[1]]

				} else if link_class == 1{
					vec![x[0], x[1]+1]

				}else{
					panic!("Megafly only route through local and global links")
				}
			},
		};

		//panic if more than 4 local hops and 2 global hops
		if cs[0] > 4
		{
			println!("cs={:?}",cs);
			panic!("MegaflyAD routing through more than 4 local hops")
		}
		if cs[1] > 2
		{
			println!("cs={:?}",cs);
			panic!("MegaflyAD routing through more than 2 global hops")
		}
		bri.selections=Some(cs);

		match bri.visited_routers
		{
			Some(ref mut v) =>
				{
					v.push(current_router);
				}
			None => panic!("visited_routers not initialized"),
		};
	}
	fn initialize(&mut self, topology:&dyn Topology, _rng: &mut StdRng)
	{
		let cartesian_data = topology.cartesian_data().expect("cartesian data not available"); //BEAWARE THAT DF+ IS AN INDIRECT NETWORK
		for i in 0..self.source_group_pattern.len()
		{
			if let Some(ref mut pattern) = self.source_group_pattern[i][0]
			{
				pattern.initialize(topology.num_servers()*topology.num_servers(), topology.num_servers()*topology.num_servers(), topology, _rng);
			}
			if let Some(ref mut pattern) = self.source_group_pattern[i][1]
			{
				pattern.initialize(cartesian_data.sides[0], cartesian_data.sides[0], topology, _rng);
			}
		}
		for i in 0..self.intermediate_group_pattern.len()
		{
			if let Some(ref mut pattern) = self.intermediate_group_pattern[i][0]
			{
				pattern.initialize(topology.num_servers()*topology.num_servers()*cartesian_data.sides[1], topology.num_servers()*topology.num_servers()*cartesian_data.sides[1], topology, _rng);
			}
			if let Some(ref mut pattern) = self.intermediate_group_pattern[i][1]
			{
				pattern.initialize(cartesian_data.sides[0], cartesian_data.sides[0], topology, _rng);
			}
		}
		for i in 0..self.destination_group_pattern.len()
		{
			if let Some(ref mut pattern) = self.destination_group_pattern[i][0]
			{
				pattern.initialize(topology.num_servers()*topology.num_servers(), topology.num_servers()*topology.num_servers(), topology, _rng);
			}
			if let Some(ref mut pattern) = self.destination_group_pattern[i][1]
			{
				pattern.initialize(cartesian_data.sides[0], cartesian_data.sides[0], topology, _rng);
			}
		}
		for i in 0..self.global_pattern_per_hop.len()
		{
			if let Some(ref mut pattern) = self.global_pattern_per_hop[i][0]
			{
				pattern.initialize(topology.num_servers()*topology.num_servers(), topology.num_servers()*topology.num_servers(), topology, _rng);
			}
			if let Some(ref mut pattern) = self.global_pattern_per_hop[i][1]
			{
				pattern.initialize(cartesian_data.sides[1], cartesian_data.sides[1], topology, _rng);
			}
		}
		// if let Some(ref mut pattern) = self.intermediate_source_minimal_pattern
		// {
		// 	pattern.initialize(topology.num_servers(), topology.num_servers(), topology, _rng);
		// }
		// if let Some(ref mut pattern) = self.intermediate_target_minimal_pattern
		// {
		// 	pattern.initialize(topology.num_servers(), topology.num_servers(), topology, _rng);
		// }
		// let cartesian_data = topology.cartesian_data().expect("cartesian data not available"); //BEAWARE THAT DF+ IS AN INDIRECT NETWORK
		// self.intermediate_leaf_switch_pattern.initialize(cartesian_data.sides[0], cartesian_data.sides[0], topology, _rng);

	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>, _num_virtual_channels:usize, _rng:&mut StdRng)
	{
	}
}

impl MegaflyAD
{
	pub fn new(arg: RoutingBuilderArgument) -> MegaflyAD
	{
		// //let mut order=None;
		// //let mut servers_per_router=None;
		// let mut first=None;
		// let mut second=None;
		// let mut pattern: Box<dyn Pattern> = Box::new(UniformPattern::uniform_pattern(true)); //pattern to intermideate node
		// // let mut exclude_h_groups=false;
		let mut first_allowed_virtual_channels =vec![0];
		let mut second_allowed_virtual_channels =vec![1];
		let mut minimal_to_deroute=vec![0, 0, 1];
		let mut source_group_pattern= vec![];
		let mut intermediate_group_pattern= vec![];
		let mut destination_group_pattern= vec![];
		let mut global_pattern_per_hop= vec![];
		let mut consume_same_channel = false;
		let mut set_global_minimal_channel = false;
		// let mut intermediate_source_minimal_pattern=None;
		// let mut intermediate_target_minimal_pattern=None;
		// let mut intermediate_leaf_switch_pattern :Box<dyn Pattern> = new_pattern(PatternBuilderArgument{cv: &ConfigurationValue::Object("Identity".to_string(), vec![]),plugs:arg.plugs});
		match_object_panic!(arg.cv,"MegaflyAD",value,

			"first_allowed_virtual_channels" => first_allowed_virtual_channels=value.
				as_array().expect("bad value for first_reserved_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in first_reserved_virtual_channels") as usize).collect(),
			"second_allowed_virtual_channels" => second_allowed_virtual_channels=value.
				as_array().expect("bad value for second_reserved_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in second_reserved_virtual_channels") as usize).collect(),
			"minimal_to_deroute" => minimal_to_deroute=value.as_array().expect("bad value for minimal_to_deroute").iter()
				.map(|v|v.as_f64().expect("bad value in minimal_to_deroute") as usize).collect(),
			// "local_pattern_per_hop" => local_pattern_per_hop=value.as_array().expect("bad value for local_pattern_per_hop").iter()
			// 	.map(|v|v.as_array().expect("bad value for local_pattern_per_hop").iter()
			// 	.map(|p|new_optional_pattern(PatternBuilderArgument{cv:p,plugs:arg.plugs})).collect()
			// ).collect(),
			"source_group_pattern" => source_group_pattern=value.as_array().expect("bad value for source_group_pattern").iter()
				.map(|v|v.as_array().expect("bad value for source_group_pattern").iter()
				.map(|p|new_optional_pattern(PatternBuilderArgument{cv:p,plugs:arg.plugs})).collect()
			).collect(),
			"intermediate_group_pattern" => intermediate_group_pattern=value.as_array().expect("bad value for intermediate_group_pattern").iter()
				.map(|v|v.as_array().expect("bad value for intermediate_group_pattern").iter()
				.map(|p|new_optional_pattern(PatternBuilderArgument{cv:p,plugs:arg.plugs})).collect()
			).collect(),
			"destination_group_pattern" => destination_group_pattern=value.as_array().expect("bad value for destination_group_pattern").iter()
				.map(|v|v.as_array().expect("bad value for destination_group_pattern").iter()
				.map(|p|new_optional_pattern(PatternBuilderArgument{cv:p,plugs:arg.plugs})).collect()
			).collect(),
			"global_pattern_per_hop" => global_pattern_per_hop=value.as_array().expect("bad value for global_pattern_per_hop").iter()
				.map(|v|v.as_array().expect("bad value for global_pattern_per_hop").iter()
				.map(|p|new_optional_pattern(PatternBuilderArgument{cv:p,plugs:arg.plugs})).collect()
			).collect(),
			"consume_same_channel" => consume_same_channel=value.as_bool().expect("bad value for consume_same_channel"),
			"set_global_minimal_channel" => set_global_minimal_channel=value.as_bool().expect("bad value for set_global_minimal_channel"),
			// "intermediate_source_minimal_pattern" => intermediate_source_minimal_pattern=new_optional_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs}),
			// "intermediate_target_minimal_pattern" => intermediate_target_minimal_pattern=new_optional_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs}),
			// "intermediate_leaf_switch_pattern" => intermediate_leaf_switch_pattern=new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs}),
		);

		MegaflyAD{
			first_allowed_virtual_channels,
			second_allowed_virtual_channels,
			minimal_to_deroute,
			source_group_pattern,
			intermediate_group_pattern,
			destination_group_pattern,
			global_pattern_per_hop,
			consume_same_channel,
			set_global_minimal_channel,
		}
	}
}