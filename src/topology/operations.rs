
use super::prelude::*;
use crate::pattern::prelude::*;
use super::NeighbourRouterIteratorItem;
use crate::matrix::Matrix;
use crate::match_object_panic;
use crate::config_parser::ConfigurationValue;
use quantifiable_derive::Quantifiable;//the derive macro

/**
Transforms the server indices of a base topology. This does not change the indices of routers.

Example configuration:
```ignore
RemappedServersTopology{
	topology: Mesh{sides:[4,4],servers_per_router:1},
	pattern: RandomPermutation,
}
```

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


