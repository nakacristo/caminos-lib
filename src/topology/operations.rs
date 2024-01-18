
use super::prelude::*;
use super::NeighbourRouterIteratorItem;
use crate::pattern::prelude::*;
use crate::matrix::Matrix;
use crate::match_object_panic;
use crate::config_parser::ConfigurationValue;
use quantifiable_derive::Quantifiable;//the derive macro

use rand::prelude::SliceRandom;
use std::collections::{HashMap,HashSet};

/**
Transforms the server indices of a base topology. This does not change the indices of routers.

Example configuration:
```ignore
RemappedServersTopology{
	topology: Mesh{sides:[4,4],servers_per_router:1},
	pattern: RandomPermutation,
}
```

For the same concept on patterns see [RemappedNodes](crate::pattern::RemappedNodes).

**/
#[derive(Debug,Quantifiable)]
pub struct RemappedServersTopology
{
	/// Maps a server index in the base topology to the outside.
	/// It must be a permutation.
	from_base_map: Vec<usize>,
	/// Maps a server index from outside.
	/// The inverse of `from_base_map`.
	into_base_map: Vec<usize>,
	/// The base topology.
	topology: Box<dyn Topology>,
}

impl Topology for RemappedServersTopology
{
	fn num_routers(&self) -> usize { self.topology.num_routers() }
	fn num_servers(&self) -> usize { self.topology.num_servers() }
	fn neighbour(&self, router_index:usize, port:usize) -> (Location,usize)
	{
		let (loc,link_class) = self.topology.neighbour(router_index,port);
		(self.map_location_from_base(loc),link_class)
	}
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let base_server = self.into_base_map[server_index];
		let (loc,link_class) = self.topology.server_neighbour(base_server);
		if let Location::ServerPort(_) = loc
		{
			panic!("A server is directly connected to another server.");
		}
		(loc,link_class)
	}
	fn diameter(&self) -> usize { self.topology.diameter() }
	fn distance(&self,origin:usize,destination:usize) -> usize { self.topology.distance(origin,destination) }
	fn amount_shortest_paths(&self,origin:usize,destination:usize) -> usize { self.topology.amount_shortest_paths(origin,destination) }
	fn average_amount_shortest_paths(&self) -> f32 { self.topology.average_amount_shortest_paths() }
	fn maximum_degree(&self) -> usize { self.topology.maximum_degree() }
	fn minimum_degree(&self) -> usize { self.topology.minimum_degree() }
	fn degree(&self, router_index: usize) -> usize { self.topology.degree(router_index) }
	fn ports(&self, router_index: usize) -> usize { self.topology.ports(router_index) }
	fn neighbour_router_iter<'a>(&'a self, router_index:usize) -> Box<dyn Iterator<Item=NeighbourRouterIteratorItem> + 'a>
	{
		self.topology.neighbour_router_iter(router_index)
	}
	fn cartesian_data(&self) -> Option<&CartesianData> { self.topology.cartesian_data() }
	fn coordinated_routing_record(&self, coordinates_a:&[usize], coordinates_b:&[usize], rng:Option<&mut StdRng>)->Vec<i32>
	{
		self.topology.coordinated_routing_record(coordinates_a,coordinates_b,rng)
	}
	fn is_direction_change(&self, router_index:usize, input_port: usize, output_port: usize) -> bool
	{
		self.topology.is_direction_change(router_index,input_port,output_port)
	}
	fn up_down_distance(&self,origin:usize,destination:usize) -> Option<(usize,usize)>
	{
		self.topology.up_down_distance(origin,destination)
	}
	// Noone really overrides this...
	fn bfs(&self, origin:usize, class_weight:Option<&[usize]>) -> Vec<usize>
	{
		self.topology.bfs(origin,class_weight)
	}
	// Noone really overrides this...
	fn compute_distance_matrix(&self, class_weight:Option<&[usize]>) -> Matrix<usize>
	{
		self.topology.compute_distance_matrix(class_weight)
	}
	// Noone really overrides this...
	fn floyd(&self) -> Matrix<usize>
	{
		self.topology.floyd()
	}
	// Noone really overrides this...
	fn compute_amount_shortest_paths(&self) -> (Matrix<usize>,Matrix<usize>)
	{
		self.topology.compute_amount_shortest_paths()
	}
	// Noone really overrides this...
	fn components(&self,allowed_classes:&[bool]) -> Vec<Vec<usize>>
	{
		self.topology.components(allowed_classes)
	}
	// Noone really overrides this...
	fn compute_near_far_matrices(&self) -> (Matrix<usize>,Matrix<usize>)
	{
		self.topology.compute_near_far_matrices()
	}
	// Noone really overrides this...
	fn eccentricity(&self, router_index:usize) -> usize
	{
		self.topology.eccentricity(router_index)
	}
	// Are these even correct to override?
	// fn check_adjacency_consistency(&self,amount_link_classes: Option<usize>)
	// fn write_adjacencies_to_file(&self, file:&mut File, _format:usize)->Result<(),std::io::Error>
}

impl RemappedServersTopology
{
	pub fn new(mut arg:TopologyBuilderArgument) -> RemappedServersTopology
	{
		let mut topology = None;
		let mut pattern = None;
		match_object_panic!(arg.cv, "RemappedServers", value,
			"topology" => topology = Some(new_topology(TopologyBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
			"pattern" => pattern = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
		);
		let topology = topology.expect("There were no topology in configuration of RemappedServersTopology.");
		let n = topology.num_servers();
		let mut pattern = pattern.expect("There were no pattern in configuration of RemappedServersTopology.");
		pattern.initialize(n,n,&*topology,arg.rng);
		let from_base_map : Vec<usize> = (0..n).map(|server_inside|{
			pattern.get_destination(server_inside,&*topology,arg.rng)
		}).collect();
		let mut into_base_map = vec![None;n];
		for (inside,&outside) in from_base_map.iter().enumerate()
		{
			match into_base_map[outside]
			{
				None => into_base_map[outside]=Some(inside),
				Some(already_inside) => panic!("Two inside servers ({inside} and {already_inside}) mapped to the same outside server ({outside}).",inside=inside,already_inside=already_inside,outside=outside),
			}
		}
		let into_base_map = into_base_map.iter().map(|x|x.expect("server not mapped")).collect();
		RemappedServersTopology{
			from_base_map,
			into_base_map,
			topology,
		}
	}
	// never called?
	pub fn map_location_into_base(&self,location:Location) -> Location
	{
		match location
		{
			Location::ServerPort(outside) => Location::ServerPort(self.into_base_map[outside]),
			x => x,
		}
	}
	pub fn map_location_from_base(&self,location:Location) -> Location
	{
		match location
		{
			Location::ServerPort(inside) => Location::ServerPort(self.from_base_map[inside]),
			x => x,
		}
	}
}

/**
Deletes `amount` links selected randomly. May employ a pattern to select on what switches they fault may occur.

The following example takes a 6x6 [Hamming] and breaks 30 links randomly selected with both enpoints inside a 4x4 block. The `seed` is fixed so the same fault set is employed even if the global RNG changes.
```ignore
topology: RandomLinkFaults{
	topology: Hamming{
		sides: [6,6],
		servers_per_router: 6,
	},
	amount:30,
	switch_pattern_input_size: 16,
	switch_pattern: CartesianEmbedding{
		source_sides: [4,4],
		destination_sides: [6,6],
	},
	seed: 0,
},
```
**/
#[derive(Debug,Quantifiable)]
pub struct RandomLinkFaults
{
	/// The base topology.
	topology: Box<dyn Topology>,
	//rng: Option<StdRng>,
	removed_links: HashMap< Location, Location >,
	///Cached distances. `distance_matrix.get(i,j)` is the distance from router i to router j.
	distance_matrix:Matrix<usize>,
	///amount_matrix.get(i,j) = amount of shortest paths from router i to router j
	amount_matrix:Matrix<usize>,
	///Average of the amount_matrix entries.
	average_amount: f32,
}

impl Topology for RandomLinkFaults
{
	fn num_routers(&self) -> usize { self.topology.num_routers() }
	fn num_servers(&self) -> usize { self.topology.num_servers() }
	fn neighbour(&self, router_index:usize, port:usize) -> (Location,usize)
	{
		if self.removed_links.get( &Location::RouterPort{router_index,router_port:port} ).is_none() {
			self.topology.neighbour(router_index,port)
		} else {
			(Location::None,0)
		}
	}
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		self.topology.server_neighbour(server_index)
	}
	fn diameter(&self) -> usize { self.compute_diameter() }
	fn distance(&self,origin:usize,destination:usize) -> usize {
		*self.distance_matrix.get(origin,destination)
	}
	fn amount_shortest_paths(&self,origin:usize,destination:usize) -> usize
	{
		*self.amount_matrix.get(origin,destination)
	}
	fn average_amount_shortest_paths(&self) -> f32
	{
		self.average_amount
	}
	fn degree(&self, router_index: usize) -> usize {
		self.neighbour_router_iter(router_index).filter(|NeighbourRouterIteratorItem{port_index,..}|
			self.removed_links.get( &Location::RouterPort{router_index,router_port:*port_index} ).is_none()
		).count()
	}
	fn ports(&self, router_index: usize) -> usize { self.topology.ports(router_index) }
	fn cartesian_data(&self) -> Option<&CartesianData> { self.topology.cartesian_data() }
	fn coordinated_routing_record(&self, coordinates_a:&[usize], coordinates_b:&[usize], rng:Option<&mut StdRng>)->Vec<i32>
	{
		// XXX what happens with broken links?
		self.topology.coordinated_routing_record(coordinates_a,coordinates_b,rng)
	}
	fn is_direction_change(&self, router_index:usize, input_port: usize, output_port: usize) -> bool
	{
		self.topology.is_direction_change(router_index,input_port,output_port)
	}
	fn up_down_distance(&self,origin:usize,destination:usize) -> Option<(usize,usize)>
	{
		// XXX what happens with broken links?
		self.topology.up_down_distance(origin,destination)
	}
}


impl RandomLinkFaults
{
	pub fn new(mut arg:TopologyBuilderArgument) -> RandomLinkFaults
	{
		let mut topology = None;
		let mut amount = None;
		let mut rng = None;
		let mut switch_pattern = None;
		let mut switch_pattern_input_size = None;
		match_object_panic!(arg.cv, "RandomLinkFaults", value,
			"topology" => topology = Some(new_topology(TopologyBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
			"amount" => amount = Some( value.as_i32().expect("bad value for amount") ),
			"seed" => rng = Some( value.as_rng().expect("bad value for seed") ),
			"switch_pattern" => switch_pattern = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"switch_pattern_input_size" => switch_pattern_input_size = Some( value.as_usize().expect("bad value for amount") ),
		);
		let topology = topology.expect("There were no topology in configuration of RemappedServersTopology.");
		let amount = amount.expect("Missing field amount in RandomLinkFaults.");
		let rng = rng.as_mut().unwrap_or(arg.rng);
		let n = topology.num_routers();
		let switch_set : Option<HashSet<usize>> = if let Some(mut pattern) = switch_pattern {
			let input_size = switch_pattern_input_size.unwrap_or(n);
			pattern.initialize(input_size,n,&*topology,rng);
			let mut switches = HashSet::new();
			for input in 0..input_size {
				let output = pattern.get_destination(input,&*topology,rng);
				switches.insert(output);
			}
			Some(switches)
		} else {
			None
		};
		let mut link_pool : Vec< (Location,Location) > = vec![];
		// We keep left<right to ensure to keep each link only once.
		// Assumming no loop links in a router...
		for left_router in 0..n
		{
			if switch_set.as_ref().is_some_and(|set|set.get(&left_router).is_none()) { continue; }
			for left_port in 0..topology.ports(left_router)
			{
				let (right_loc,_link_class) = topology.neighbour(left_router,left_port);
				if let Location::RouterPort{router_index,..} = right_loc {
					if switch_set.as_ref().is_some_and(|set|set.get(&router_index).is_none()) { continue; }
					if left_router < router_index {
						let left_loc = Location::RouterPort{router_index:left_router, router_port:left_port};
						link_pool.push( (left_loc,right_loc) );
					}
				}
			}
		}
		if link_pool.len() < amount as usize {
			panic!("Not enough link candidates to remove. {} candidates, {} asked to remove.",link_pool.len(),amount);
		}
		// We delete amount links.
		// It is simple to shuffle the array and get the first ones. A bit inefficient, but no relevant.
		link_pool.shuffle(rng);
		let mut removed_links = HashMap::new();
		for (left_loc,right_loc) in link_pool.into_iter().take(amount as usize) {
			removed_links.insert( left_loc.clone(), right_loc.clone() );
			removed_links.insert( right_loc, left_loc );
		}
		let mut topo = RandomLinkFaults{
			topology,
			removed_links,
			distance_matrix:Matrix::constant(0,0,0),
			amount_matrix:Matrix::constant(0,0,0),
			average_amount: 0f32,
		};
		let (distance_matrix,amount_matrix)=topo.compute_amount_shortest_paths();
		topo.distance_matrix=distance_matrix;
		topo.amount_matrix=amount_matrix;
		topo.average_amount={
			//vertex_index n=size();
			let n=topo.num_routers();
			//long r=0,count=0;
			let mut r=0;
			let mut count=0;
			//for(vertex_index i=0;i<n;i++)
			for i in 0..n
			{
				//if(!isInput(i))continue;
				//for(vertex_index j=0;j<n;j++)
				for j in 0..n
				{
					//if(!isOutput(j) || i==j)continue;
					if i!=j
					{
						r+=topo.amount_shortest_paths(i,j);
						count+=1;
					}
				}
			}
			//return (double)r/(double)count;
			r as f32/count as f32
		};
		topo
	}
}



