
use crate::pattern::probabilistic::UniformPattern;
use std::cell::RefCell;
use ::rand::{Rng,rngs::StdRng};
use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
//use topology::{Topology,Location,NeighbourRouterIteratorItem,TopologyBuilderArgument,new_topology};
use super::prelude::*;
//use crate::routing::{RoutingInfo,Routing,CandidateEgress,RoutingBuilderArgument,RoutingNextCandidates};
use crate::routing::prelude::*;
use crate::matrix::Matrix;
use crate::match_object_panic;
use crate::routing::RoutingAnnotation;
use crate::pattern::*; //For Valiant

//extern crate itertools;
use itertools::Itertools;
use rand::SeedableRng;
use crate::quantify::Quantifiable;

///A Cartesian ortahedral region of arbitrary dimension.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct CartesianData
{
	pub sides: Vec<usize>,
	pub size: usize,
}

impl CartesianData
{
	pub fn new(sides:&[usize]) -> CartesianData
	{
		CartesianData{
			sides:sides.to_vec(),
			size: sides.iter().product(),
		}
	}
	pub fn unpack(&self, mut router_index: usize) -> Vec<usize>
	{
		//let mut stride=self.size;
		if router_index>=self.size
		{
			panic!("router_index={} is greater than the size of the CartesianData={}",router_index,self.size);
		}
		let mut r=Vec::with_capacity(self.sides.len());
		for side in self.sides.iter()
		{
			//stride/=side;
			//r.push(router_index%stride);
			//router_index/=side;
			r.push(router_index%side);
			router_index/=side;
		}
		r
	}
	pub fn pack(&self, coordinates:&[usize]) -> usize
	{
		//check that the coordinates are within the sides
		for (c,s) in coordinates.iter().zip(self.sides.iter())
		{
			if *c>=*s
			{
				panic!("coordinate {} is greater than the side {}",c,s);
			}
		}
		let mut r=0;
		let mut stride=1;
		for (i,side) in self.sides.iter().enumerate()
		{
			r+=coordinates[i]*stride;
			stride*=side;
		}
		r
	}
}

///The mesh topology, a rectangle with corners.
///Its maximum_degree is the double of the dimension, with boundary routers having less degree.
///The ports that would go outside the mesh have `None` as neighbour.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Mesh
{
	cartesian_data: CartesianData,
	servers_per_router: usize,
}

//impl Quantifiable for Mesh
//{
//	fn total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//	fn print_memory_breakdown(&self)
//	{
//		unimplemented!();
//	}
//	fn forecast_total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//}

impl Topology for Mesh
{
	fn num_routers(&self) -> usize
	{
		self.cartesian_data.size
	}
	fn num_servers(&self) -> usize
	{
		self.cartesian_data.size*self.servers_per_router
	}
	//fn num_arcs(&self) -> usize
	//{
	//	self.num_routers()*self.cartesian_data.sides.len()*2
	//}
	fn neighbour(&self, router_index:usize, port: usize) -> (Location,usize)
	{
		let m=self.cartesian_data.sides.len();
		if port<2*m
		{
			let dimension=port/2;
			let delta=if port%2==0 { -1i32 as usize } else { 1 };
			let mut coordinates=self.cartesian_data.unpack(router_index);

			//mesh
			coordinates[dimension]=coordinates[dimension].wrapping_add(delta);
			if coordinates[dimension]>=self.cartesian_data.sides[dimension]
			{
				return (Location::None,0);
			}

			//torus
			//let side=self.cartesian_data.sides[dimension];
			//coordinates[dimension]=(coordinates[dimension]+side.wrapping_add(delta))%side;

			let n_index=self.cartesian_data.pack(&coordinates);
			let n_port= if delta==1
			{
				dimension*2
			}
			else
			{
				dimension*2+1
			};
			return (Location::RouterPort{router_index:n_index, router_port:n_port},dimension);
		}
		(Location::ServerPort(port-2*m + router_index*self.servers_per_router),m)
	}
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let m=self.cartesian_data.sides.len();
		(Location::RouterPort{
			router_index: server_index/self.servers_per_router,
			router_port: 2*m+server_index%self.servers_per_router,
		},m)
	}
	fn diameter(&self) -> usize
	{
		self.cartesian_data.sides.iter().map(|s|s-1).sum()
	}
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		let coord_origin=self.cartesian_data.unpack(origin);
		let coord_destination=self.cartesian_data.unpack(destination);
		let rr=self.coordinated_routing_record(&coord_origin,&coord_destination,None);
		rr.iter().map(|x|x.abs() as usize).sum()
	}
	fn amount_shortest_paths(&self,_origin:usize,_destination:usize) -> usize
	{
		unimplemented!();
	}
	fn average_amount_shortest_paths(&self) -> f32
	{
		unimplemented!();
	}
	fn maximum_degree(&self) -> usize
	{
		2*self.cartesian_data.sides.len()
	}
	fn minimum_degree(&self) -> usize
	{
		self.cartesian_data.sides.len()
	}
	fn degree(&self, router_index: usize) -> usize
	{
		let coordinates=self.cartesian_data.unpack(router_index);
		let mut d=coordinates.len();
		for (i,c) in coordinates.iter().enumerate()
		{
			if *c!=0 && *c!=self.cartesian_data.sides[i]-1
			{
				d+=1;
			}
		}
		d
	}
	fn ports(&self, _router_index: usize) -> usize
	{
		2*self.cartesian_data.sides.len()+self.servers_per_router
	}
	fn cartesian_data(&self) -> Option<&CartesianData>
	{
		Some(&self.cartesian_data)
	}
	fn coordinated_routing_record(&self, coordinates_a:&[usize], coordinates_b:&[usize], _rng: Option<&mut StdRng>)->Vec<i32>
	{
		//In a Mesh the routing record is just the difference in coordinates.
		(0..coordinates_a.len()).map(|i|coordinates_b[i] as i32-coordinates_a[i] as i32).collect()
	}
	fn is_direction_change(&self, _router_index:usize, input_port: usize, output_port: usize) -> bool
	{
		input_port/2 != output_port/2
	}
	fn up_down_distance(&self,_origin:usize,_destination:usize) -> Option<(usize,usize)>
	{
		None
	}
}

impl Mesh
{
	pub fn new(cv:&ConfigurationValue) -> Mesh
	{
		let mut sides:Option<Vec<_>>=None;
		let mut servers_per_router=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=cv
		{
			if cv_name!="Mesh"
			{
				panic!("A Mesh must be created from a `Mesh` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"sides" => match value
					{
						&ConfigurationValue::Array(ref a) => sides=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in sides"),
						}).collect()),
						_ => panic!("bad value for sides"),
					}
					"servers_per_router" => match value
					{
						&ConfigurationValue::Number(f) => servers_per_router=Some(f as usize),
						_ => panic!("bad value for servers_per_router"),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in Mesh",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a Mesh from a non-Object");
		}
		let sides=sides.expect("There were no sides");
		let servers_per_router=servers_per_router.expect("There were no servers_per_router");
		//println!("servers_per_router={}",servers_per_router);
		Mesh{
			cartesian_data: CartesianData::new(&sides),
			servers_per_router,
		}
	}
}

///As the mesh but with 'wrap-around' links. This is a regular topology and there is no port to `None`.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Torus
{
	cartesian_data: CartesianData,
	servers_per_router: usize,
}

//impl Quantifiable for Torus
//{
//	fn total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//	fn print_memory_breakdown(&self)
//	{
//		unimplemented!();
//	}
//	fn forecast_total_memory(&self) -> usize
//	{
//		unimplemented!();
//	}
//}

impl Topology for Torus
{
	fn num_routers(&self) -> usize
	{
		self.cartesian_data.size
	}
	fn num_servers(&self) -> usize
	{
		self.cartesian_data.size*self.servers_per_router
	}
	//fn num_arcs(&self) -> usize
	//{
	//	self.num_routers()*self.cartesian_data.sides.len()*2
	//}
	//fn num_servers(&self, _router_index:usize) -> usize
	//{
	//	self.servers_per_router
	//}
	fn neighbour(&self, router_index:usize, port: usize) -> (Location,usize)
	{
		let m=self.cartesian_data.sides.len();
		if port<2*m
		{
			let dimension=port/2;
			let delta=if port%2==0 { -1i32 as usize } else { 1 };
			let mut coordinates=self.cartesian_data.unpack(router_index);
			//coordinates[dimension]=coordinates[dimension].wrapping_add(delta);
			//if coordinates[dimension]>=self.cartesian_data.sides[dimension]
			//{
			//	return Location::None;
			//}
			let side=self.cartesian_data.sides[dimension];
			//coordinates[dimension]=(coordinates[dimension]+side+delta)%side;
			coordinates[dimension]=(coordinates[dimension]+side.wrapping_add(delta))%side;
			let n_index=self.cartesian_data.pack(&coordinates);
			let n_port= if delta==1
			{
				dimension*2
			}
			else
			{
				dimension*2+1
			};
			return (Location::RouterPort{router_index:n_index, router_port:n_port},dimension);
		}
		(Location::ServerPort(port-2*m + router_index*self.servers_per_router),m)
	}
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let m=self.cartesian_data.sides.len();
		(Location::RouterPort{
			router_index: server_index/self.servers_per_router,
			router_port: 2*m+server_index%self.servers_per_router,
		},m)
	}
	fn diameter(&self) -> usize
	{
		self.cartesian_data.sides.iter().map(|s|s/2).sum()
	}
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		let coord_origin=self.cartesian_data.unpack(origin);
		let coord_destination=self.cartesian_data.unpack(destination);
		let rr=self.coordinated_routing_record(&coord_origin,&coord_destination,None);
		rr.iter().map(|x|x.abs() as usize).sum()
	}
	fn amount_shortest_paths(&self,_origin:usize,_destination:usize) -> usize
	{
		unimplemented!();
	}
	fn average_amount_shortest_paths(&self) -> f32
	{
		unimplemented!();
	}
	fn maximum_degree(&self) -> usize
	{
		2*self.cartesian_data.sides.len()
	}
	fn minimum_degree(&self) -> usize
	{
		2*self.cartesian_data.sides.len()
	}
	fn degree(&self, _router_index: usize) -> usize
	{
		2*self.cartesian_data.sides.len()
	}
	fn ports(&self, _router_index: usize) -> usize
	{
		2*self.cartesian_data.sides.len()+self.servers_per_router
	}
	fn cartesian_data(&self) -> Option<&CartesianData>
	{
		Some(&self.cartesian_data)
	}
	fn coordinated_routing_record(&self, coordinates_a:&[usize], coordinates_b:&[usize], mut rng: Option<&mut StdRng>)->Vec<i32>
	{
		//In a Torus the routing record is for every difference of coordinates `d`, the minimum among `d` and `side-d` with the appropiate sign.
		(0..coordinates_a.len()).map(|i|{
			//coordinates_b[i] as i32-coordinates_a[i] as i32
			let side=self.cartesian_data.sides[i] as i32;
			let a=(side + coordinates_b[i] as i32-coordinates_a[i] as i32) % side;
			let b=(side + coordinates_a[i] as i32-coordinates_b[i] as i32) % side;
			if a==b
			{
				if let Some(ref mut rng)=rng
				{
					//let r=rng.gen_range(0,2);//rand-0.4
					let r=rng.gen_range(0..2);//rand-0.8
					if r==0 { a } else { -b }
				}
				else
				{
					a
				}
			}
			else if a<b { a } else { -b }
		}).collect()
	}
	fn is_direction_change(&self, _router_index:usize, input_port: usize, output_port: usize) -> bool
	{
		input_port/2 != output_port/2
	}
	fn up_down_distance(&self,_origin:usize,_destination:usize) -> Option<(usize,usize)>
	{
		None
	}
}

impl Torus
{
	pub fn new(cv:&ConfigurationValue) -> Torus
	{
		let mut sides:Option<Vec<_>>=None;
		let mut servers_per_router=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=cv
		{
			if cv_name!="Torus"
			{
				panic!("A Torus must be created from a `Torus` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"sides" => match value
					{
						&ConfigurationValue::Array(ref a) => sides=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in sides"),
						}).collect()),
						_ => panic!("bad value for sides"),
					}
					"servers_per_router" => match value
					{
						&ConfigurationValue::Number(f) => servers_per_router=Some(f as usize),
						_ => panic!("bad value for servers_per_router"),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in Torus",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a Torus from a non-Object");
		}
		let sides=sides.expect("There were no sides");
		let servers_per_router=servers_per_router.expect("There were no servers_per_router");
		//println!("servers_per_router={}",servers_per_router);
		Torus{
			cartesian_data: CartesianData::new(&sides),
			servers_per_router,
		}
	}
}

///The Hamming graph, the Cartesian product of complete graphs.
///Networks based on Hamming graphs have been called flattened butterflies and Hyper X.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Hamming
{
	cartesian_data: CartesianData,
	servers_per_router: usize,
	wiring: Vec<Box<dyn CompleteGraphWiring>>,
}

impl Topology for Hamming
{
	fn num_routers(&self) -> usize
	{
		self.cartesian_data.size
	}
	fn num_servers(&self) -> usize
	{
		self.cartesian_data.size*self.servers_per_router
	}
	//fn num_arcs(&self) -> usize
	//{
	//	self.num_routers()*self.maximum_degree()
	//}
	//fn num_servers(&self, _router_index:usize) -> usize
	//{
	//	self.servers_per_router
	//}
	fn neighbour(&self, router_index:usize, port: usize) -> (Location,usize)
	{
		let m=self.cartesian_data.sides.len();
		let mut dimension=0;
		let mut offset=port;
		while dimension<m && offset>=self.cartesian_data.sides[dimension]-1
		{
			offset-=self.cartesian_data.sides[dimension]-1;
			dimension+=1;
		}
		if dimension<m
		{
			let mut coordinates=self.cartesian_data.unpack(router_index);
			let (dest_switch_dim, dest_port)= self.wiring[dimension].map(coordinates[dimension], offset);
			coordinates[dimension]=dest_switch_dim;
			//Print all data for debugging
			// println!("router_index: {} -> port: {} -> dest_switch_dim: {} -> coordinates[dimension]: {} -> dest_port:{}",router_index,port,coordinates[dimension],dest_switch_dim,dest_port);
			return (Location::RouterPort{
				router_index: self.cartesian_data.pack(&coordinates),
				router_port: (port-offset) + dest_port,
			},dimension);
			// let side=self.cartesian_data.sides[dimension];
			// coordinates[dimension]=(coordinates[dimension]+offset+1)%side;
			// let n_index=self.cartesian_data.pack(&coordinates);
			// let n_port= (side-2-offset) + (port-offset);
			// return (Location::RouterPort{router_index:n_index, router_port:n_port},dimension);
		}
		(Location::ServerPort(offset + router_index*self.servers_per_router),m)
	}
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let m=self.cartesian_data.sides.len();
		(Location::RouterPort{
			router_index: server_index/self.servers_per_router,
			router_port: self.maximum_degree()+server_index%self.servers_per_router,
		},m)
	}
	fn diameter(&self) -> usize
	{
		self.cartesian_data.sides.len()
	}
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		let m=self.cartesian_data.sides.len();
		let mut d=0;
		let co=self.cartesian_data.unpack(origin);
		let cd=self.cartesian_data.unpack(destination);
		for i in 0..m
		{
			if co[i]!=cd[i]
			{
				d+=1;
			}
		}
		d
	}
	fn amount_shortest_paths(&self,_origin:usize,_destination:usize) -> usize
	{
		unimplemented!();
	}
	fn average_amount_shortest_paths(&self) -> f32
	{
		unimplemented!();
	}
	fn maximum_degree(&self) -> usize
	{
		self.cartesian_data.sides.iter().fold(0usize,|accumulator,x|accumulator+x-1)
	}
	fn minimum_degree(&self) -> usize
	{
		self.maximum_degree()
	}
	fn degree(&self, _router_index: usize) -> usize
	{
		self.maximum_degree()
	}
	fn ports(&self, _router_index: usize) -> usize
	{
		self.maximum_degree()+self.servers_per_router
	}
	fn cartesian_data(&self) -> Option<&CartesianData>
	{
		Some(&self.cartesian_data)
	}
	fn coordinated_routing_record(&self, coordinates_a:&[usize], coordinates_b:&[usize], _rng: Option<&mut StdRng>)->Vec<i32>
	{
		//In Hamming we put the difference as in the mesh, but any number can be advanced in a single hop.
		(0..coordinates_a.len()).map(|i|coordinates_b[i] as i32-coordinates_a[i] as i32).collect()
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

impl Hamming
{
	pub fn new(cv:&ConfigurationValue) -> Hamming
	{
		let mut sides:Option<Vec<_>>=None;
		let mut servers_per_router=None;
		let mut wiring_str= ConfigurationValue::Object("CompleteGraphRelative".to_string(), vec![]); // Box::new(CompleteGraphRelative::default());
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=cv
		{
			if cv_name!="Hamming"
			{
				panic!("A Hamming must be created from a `Hamming` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"sides" => match value
					{
						&ConfigurationValue::Array(ref a) => sides=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in sides"),
						}).collect()),
						_ => panic!("bad value for sides"),
					}
					"servers_per_router" => match value
					{
						&ConfigurationValue::Number(f) => servers_per_router=Some(f as usize),
						_ => panic!("bad value for servers_per_router"),
					}
					"wiring" => match value
					{
						&ConfigurationValue::Object(ref cv_name, ref _cv_pairs) => wiring_str= ConfigurationValue::Object(cv_name.clone(),cv_pairs.clone()),//new_complete_graph_wiring(ConfigurationValue::Object(cv_name.clone(),cv_pairs.clone())),
						_ => todo!(),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in Hamming",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a Hamming from a non-Object");
		}
		let sides=sides.expect("There were no sides");
		//TODO if sides are of different sizes
		// assert_eq!(sides.iter().unique().count(),1,"All sides must be the same");

		let cartesian_data=CartesianData::new(&sides);
		let servers_per_router=servers_per_router.expect("There were no servers_per_router");
		let wiring = (0..cartesian_data.sides.len()).map(|i|
			{
				let mut w=new_complete_graph_wiring(wiring_str.clone());
				w.initialize(cartesian_data.sides[i], &mut StdRng::from_entropy());
				w
			}
		).collect();
		// wiring.initialize(cartesian_data.sides[0], &mut StdRng::from_entropy());
		//println!("servers_per_router={}",servers_per_router);
		Hamming{
			cartesian_data,
			servers_per_router,
			wiring,
		}
	}
}

pub trait CompleteGraphWiring : Quantifiable + core::fmt::Debug
{
	/// Initialization should be called once before any other of its methods.
	fn initialize(&mut self, size:usize, rng: &mut StdRng);
	/// Gets the point connected to the `input`.
	fn map( &self, switch:usize, port:usize ) -> (usize,usize);
	/// Get the size with the arrangement has been initialized.
	fn get_size(&self) -> usize;
}

#[derive(Quantifiable,Debug,Default)]
pub struct CompleteGraphRelative{
	switches: usize,
}

impl CompleteGraphWiring for CompleteGraphRelative
{
	fn initialize(&mut self, size:usize, _rng: &mut StdRng)
	{
		self.switches=size;
	}
	fn map( &self, switch:usize, port:usize) -> (usize,usize)
	{
		((switch + port + 1) % self.switches, (self.switches -1 -port -1) % self.switches )
	}

	fn get_size(&self) -> usize
	{
		self.switches
	}
}


pub fn new_complete_graph_wiring(arg:ConfigurationValue) -> Box<dyn CompleteGraphWiring>
{
	if let ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg
	{
		match cv_name.as_ref()
		{
			"CompleteGraphRelative" => Box::new(CompleteGraphRelative::default()),
			_ => panic!("Unknown complete graph wiring {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create an arrangement from a non-Object");
	}
}

/**
Gives a Cartesian representation to a topology, providing the `cartesian_data` method.
However, does not provide of `coordinated_routing_record`.
This can be used, for example, to declare the sides of a file-given topology.

```ignore
AsCartesianTopology{
	topology: File{ filename:"/path/topo_with_100_switches", format:0, servers_per_router:5 }
	sides: [10,10],
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct AsCartesianTopology
{
	pub topology: Box<dyn Topology>,
	pub cartesian_data: CartesianData,
}

impl Topology for AsCartesianTopology
{
	fn num_routers(&self) -> usize { self.topology.num_routers() }
	fn num_servers(&self) -> usize { self.topology.num_servers() }
	fn neighbour(&self, router_index:usize, port:usize) -> (Location,usize) { self.topology.neighbour(router_index,port) }
	fn server_neighbour(&self, server_index:usize) -> (Location,usize) { self.topology.server_neighbour(server_index) }
	fn diameter(&self) -> usize { self.topology.diameter() }
	fn distance(&self,origin:usize,destination:usize) -> usize { self.topology.distance(origin,destination) }
	fn amount_shortest_paths(&self,origin:usize,destination:usize) -> usize { self.topology.distance(origin,destination) }
	fn average_amount_shortest_paths(&self) -> f32 { self.topology.average_amount_shortest_paths() }
	fn maximum_degree(&self) -> usize { self.topology.maximum_degree() }
	fn minimum_degree(&self) -> usize { self.topology.minimum_degree() }
	fn degree(&self, router_index: usize) -> usize { self.topology.degree(router_index) }
	fn ports(&self, router_index: usize) -> usize { self.topology.ports(router_index) }
	fn neighbour_router_iter<'a>(&'a self, router_index:usize) -> Box<dyn Iterator<Item=NeighbourRouterIteratorItem> + 'a>
	{ self.topology.neighbour_router_iter(router_index) }
	fn cartesian_data(&self) -> Option<&CartesianData>
	{
		Some(&self.cartesian_data)
	}
	fn coordinated_routing_record(&self, coordinates_a:&[usize], coordinates_b:&[usize], rng:Option<&mut StdRng>)->Vec<i32>
	{ self.topology.coordinated_routing_record(coordinates_a,coordinates_b,rng) }
	fn is_direction_change(&self, _router_index:usize, _input_port: usize, _output_port: usize) -> bool {
		todo!()
		// This seems it should be implemented
	}
	fn up_down_distance(&self,origin:usize,destination:usize) -> Option<(usize,usize)>
	{ self.topology.up_down_distance(origin,destination) }
	fn dragonfly_size(&self) -> Option<crate::topology::dragonfly::ArrangementSize>
	{ self.topology.dragonfly_size() }
	fn bfs(&self, origin:usize, class_weight:Option<&[usize]>) -> Vec<usize>
	{ self.topology.bfs(origin,class_weight) }
	fn compute_distance_matrix(&self, class_weight:Option<&[usize]>) -> Matrix<usize>
	{ self.topology.compute_distance_matrix(class_weight) }
	fn compute_amount_shortest_paths(&self) -> (Matrix<usize>,Matrix<usize>)
	{ self.topology.compute_amount_shortest_paths() }
	fn components(&self,allowed_classes:&[bool]) -> Vec<Vec<usize>>
	{ self.topology.components(allowed_classes) }
	fn compute_near_far_matrices(&self) -> (Matrix<usize>,Matrix<usize>)
	{ self.topology.compute_near_far_matrices() }
	fn eccentricity(&self, router_index:usize) -> usize
	{ self.topology.eccentricity(router_index) }
}

impl AsCartesianTopology
{
	pub fn new(mut arg:TopologyBuilderArgument) -> AsCartesianTopology
	{
		let mut sides:Option<Vec<_>>=None;
		let mut topology:Option<Box<dyn Topology>> = None;
		match_object_panic!(arg.cv,"AsCartesianTopology",value,
			"sides" => sides = Some(value.as_array().expect("bad value for sides").iter().map(|v|v.as_usize().expect("bad value in sides")).collect()),
			"topology" => topology = Some(new_topology(arg.with_cv(value))),
		);
		let sides=sides.expect("There were no sides");
		let topology=topology.expect("There were no topology");
		let nr = topology.num_routers();
		let sm = sides.iter().product();
		assert_eq!(nr,sm,"size of the topology does not match the given sides.");
		AsCartesianTopology{
			cartesian_data: CartesianData::new(&sides),
			topology,
		}
	}
}

//struct CartesianRoutingRecord
//{
//	coordinates: Vec<usize>,
//}

///A shortest routing for Cartesian topologies employing links in a predefined order.
///This is, if `order=[0,1]` the packet will go first by links changing the 0-dimension and then it will use the links in the 1-dimension until destination.
///The amount of links in each dimension is stored in `routing_info.routing_record` when the packet reaches the first routing and it is updated each hop.
#[derive(Debug)]
pub struct DOR
{
	order: Vec<usize>,
}

//impl RoutingInfo for CartesianRoutingRecord
//{
//	//type routing=DOR;
//}

impl Routing for DOR
{
	//type info=CartesianRoutingRecord;
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, _target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, _rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//let routing_record=&routing_info.routing_record.expect("DOR requires a routing record");
		let routing_record=if let Some(ref rr)=routing_info.routing_record
		{
			rr
		}
		else
		{
			panic!("DOR requires a routing record");
		};
		let m=routing_record.len();
		let mut i=0;
		while i<m && routing_record[self.order[i]]==0
		{
			i+=1;
		}
		if i==m
		{
			//To server
			let target_server = target_server.expect("target server was not given.");
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						//return vec![i];
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						let r= (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return Ok(RoutingNextCandidates{candidates:r,idempotent:true});
					}
				}
			}
			panic!("The server {} is not attached to this router ({}) but the routing record is {:?}",target_server,current_router,routing_record);
		}
		else
		{
			i=self.order[i];
			//Go in dimension i
			// //WARNING: This assumes ports in a mesh-like configuration!
			// if routing_record[i]<0
			// {
			// 	//return vec![2*i];
			// 	return (0..num_virtual_channels).map(|vc|(2*i,vc)).collect();
			// }
			// else
			// {
			// 	//return vec![2*i+1];
			// 	return (0..num_virtual_channels).map(|vc|(2*i+1,vc)).collect();
			// }
			//let (target_location,_link_class)=topology.server_neighbour(target_server);
			//let target_router=match target_location
			//{
			//	Location::RouterPort{router_index,router_port:_} =>router_index,
			//	_ => panic!("The server is not attached to a router"),
			//};
			let cartesian_data=topology.cartesian_data().expect("DOR requires a Cartesian topology");
			let up_current=cartesian_data.unpack(current_router);
			//let up_target=cartesian_data.unpack(target_router);
			let mut best=vec![];
			let mut best_amount=0;
			let limit=routing_record[i].abs() as usize;
			let side=cartesian_data.sides[i];
			for j in 0..topology.ports(current_router)
			{
				if let (Location::RouterPort{router_index: next_router, router_port:_},next_link_class)=topology.neighbour(current_router,j)
				{
					if next_link_class==i
					{
						let up_next=cartesian_data.unpack(next_router);
						//if up_target[i]==up_next[i]
						//{
						// 	return (0..num_virtual_channels).map(|vc|(j,vc)).collect();
						//}
						let amount=(if routing_record[i]<0
						{
							up_current[i]-up_next[i]
						}
						else
						{
							up_next[i]-up_current[i]
						}+side)%side;
						if amount<=limit
						{
							if amount>best_amount
							{
								best_amount=amount;
								best=vec![j];
							}
							else if amount==best_amount
							{
								best.push(j);
							}
						}
					}
				}
			}
			if best.is_empty()
			{
				panic!("No links improving {} dimension\n",i);
			}
			//return (0..num_virtual_channels).flat_map(|vc| best.iter().map(|p|(*p,vc)).collect::<Vec<(usize,usize)>>()).collect();
			let r= (0..num_virtual_channels).flat_map(|vc| best.iter().map(|p|CandidateEgress::new(*p,vc)).collect::<Vec<_>>()).collect();
			return Ok(RoutingNextCandidates{candidates:r,idempotent:true});
		}
	}
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, toology:&dyn Topology, current_router:usize, target_server:usize)
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, _target_server:Option<usize>, rng: &mut StdRng)
	{
		//let (target_location,_link_class)=topology.server_neighbour(target_server);
		//let target_router=match target_location
		//{
		//	Location::RouterPort{router_index,router_port:_} =>router_index,
		//	_ => panic!("The server is not attached to a router"),
		//};
		//DOR needs cartesian data in the topology, which could be a dragonfly or whatever...
		let cartesian_data=topology.cartesian_data().expect("DOR requires a Cartesian topology");
		let up_current=cartesian_data.unpack(current_router);
		let up_target=cartesian_data.unpack(target_router);
		//let routing_record=(0..up_current.len()).map(|i|up_target[i] as i32-up_current[i] as i32).collect();//FIXME: torus
		let routing_record=topology.coordinated_routing_record(&up_current,&up_target,Some(rng));
		//println!("routing record from {} to {} is {:?}",current_router,target_router,routing_record);
		routing_info.borrow_mut().routing_record=Some(routing_record);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, _target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		//let dimension=current_port/2;
		//let delta=if current_port%2==0 { -1i32 } else { 1i32 };
		let cartesian_data=topology.cartesian_data().expect("DOR requires a Cartesian topology");
		if let (Location::RouterPort{router_index: previous_router, router_port:_},dimension)=topology.neighbour(current_router,current_port)
		{
			let up_current=cartesian_data.unpack(current_router);
			let up_previous=cartesian_data.unpack(previous_router);
			let side=cartesian_data.sides[dimension] as i32;
			match routing_info.borrow_mut().routing_record
			{
				Some(ref mut rr) =>
				{
					let delta:i32=if rr[dimension]<0
					{
						(up_previous[dimension] as i32 - up_current[dimension] as i32 + side)%side
					}
					else
					{
						-((up_current[dimension] as i32 - up_previous[dimension] as i32 + side)%side)
					};
					rr[dimension]+=delta;
					// --- DEBUG vvv
					//let (target_location,_link_class)=topology.server_neighbour(target_server);
					//let target_router=match target_location
					//{
					//	Location::RouterPort{router_index,router_port:_} =>router_index,
					//	_ => panic!("The server is not attached to a router"),
					//};
					//let up_target=cartesian_data.unpack(target_router);
					//println!("new routing record. current_router={}({:?}, current_port={} previous_router={}({:?}), delta={}, rr={:?}, target_server={} target_router={}({:?})",current_router,up_current,current_port,previous_router,up_previous,delta,rr,target_server,target_router,up_target);
					// --- DEBUG ^^^
				},
				None => panic!("trying to update without routing_record"),
			};
		}
		else
		{
			panic!("!!");
		}
	}
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &mut StdRng)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>, _num_virtual_channels:usize, _rng:&mut StdRng)
	{
	}
	fn statistics(&self, _cycle:Time) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:Time)
	{
	}
}

impl DOR
{
	pub fn new(arg:RoutingBuilderArgument) -> DOR
	{
		let mut order=None;
		//let mut servers_per_router=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="DOR"
			{
				panic!("A DOR must be created from a `DOR` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"order" => match value
					{
						&ConfigurationValue::Array(ref a) => order=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in order"),
						}).collect()),
						_ => panic!("bad value for order"),
					}
					//"servers_per_router" => match value
					//{
					//	&ConfigurationValue::Number(f) => servers_per_router=Some(f as usize),
					//	_ => panic!("bad value for servers_per_router"),
					//}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in DOR",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a DOR from a non-Object");
		}
		//let sides=sides.expect("There were no sides");
		//let servers_per_router=servers_per_router.expect("There were no servers_per_router");
		let order=order.expect("There were no order");
		DOR{
			order,
		}
	}
}

/// Valiant DOR. Proposed by Valiant for Multidimensional grids. Generally you should randomize n-1 dimensions, thereby employing shortest routes when the topology is just a path.
/// `routing_info.selections=Some([k,r])` indicates that the `next` call should go toward `r` at dimension `randomized[k]`. `r` having been selected randomly previously.
/// `routing_info.selections=None` indicates to behave as DOR.
///
/// It should not be confused with Valiant's general strategy of routing through a random intermediate, that may use DOR for those sections.
#[derive(Debug)]
pub struct ValiantDOR
{
	/// Dimensions in which to ranomize.
	/// Valiant proposed to randomize the last n-1 dimensions from last to second. (randomized=[n-1,n-2,...,2,1]).
	randomized: Vec<usize>,
	/// Dimensions in which to minimally reduce the routing record.
	/// Valiant proposed to correct all the dimensions starting from the first. (shortest=[0,1,...,n-1]).
	shortest: Vec<usize>,
	/// Virtual channels reserved exclusively for the randomization.
	randomized_reserved_virtual_channels: Vec<usize>,
	/// Virtual channels reserved exclusively for the shortest routes.
	shortest_reserved_virtual_channels: Vec<usize>,
}

impl Routing for ValiantDOR
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, _target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, _rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//let routing_record=&routing_info.routing_record.expect("ValiantDOR requires a routing record");
		let mut random_amount=0i32;
		let mut be_random=false;
		let randomized_offset=if let Some(ref v)=routing_info.selections
		{
			random_amount=v[1];
			be_random=true;
			Some(v[0] as usize)
		}
		else
		{
			None
		};
		let routing_record=if let Some(ref rr)=routing_info.routing_record
		{
			rr
		}
		else
		{
			panic!("ValiantDOR requires a routing record");
		};
		let m=routing_record.len();
		let mut first_bad=0;
		while first_bad<m && routing_record[self.shortest[first_bad]]==0
		{
			first_bad+=1;
		}
		if first_bad==m
		{
			//To server
			let target_server = target_server.expect("target server was not given.");
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						//return vec![i];
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						let r= (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return Ok(RoutingNextCandidates{candidates:r,idempotent:true});
					}
				}
			}
			panic!("The server {} is not attached to this router ({}) but the routing record is {:?}",target_server,current_router,routing_record);
		}
		else
		{
			let dim=if let Some(k)=randomized_offset
			{
				self.randomized[k]
			}
			else
			{
				self.shortest[first_bad]
			};
			//Go in dimension dim
			// //WARNING: This assumes ports in a mesh-like configuration!
			// if routing_record[dim]<0
			// {
			// 	//return vec![2*dim];
			// 	return (0..num_virtual_channels).map(|vc|(2*dim,vc)).collect();
			// }
			// else
			// {
			// 	//return vec![2*dim+1];
			// 	return (0..num_virtual_channels).map(|vc|(2*dim+1,vc)).collect();
			// }
			//let (target_location,_link_class)=topology.server_neighbour(target_server);
			//let target_router=match target_location
			//{
			//	Location::RouterPort{router_index,router_port:_} =>router_index,
			//	_ => panic!("The server is not attached to a router"),
			//};
			let cartesian_data=topology.cartesian_data().expect("ValiantDOR requires a Cartesian topology");
			let up_current=cartesian_data.unpack(current_router);
			//let up_target=cartesian_data.unpack(target_router);
			let mut best=vec![];
			let mut best_amount=0;
			//let limit=routing_record[dim].abs() as usize;
			let target_amount=if be_random
			{
				random_amount
			}
			else
			{
				routing_record[dim]
			};
			let limit=target_amount.abs() as usize;
			let side=cartesian_data.sides[dim];
			for j in 0..topology.ports(current_router)
			{
				if let (Location::RouterPort{router_index: next_router, router_port:_},next_link_class)=topology.neighbour(current_router,j)
				{
					if next_link_class==dim
					{
						let up_next=cartesian_data.unpack(next_router);
						//if up_target[dim]==up_next[dim]
						//{
						// 	return (0..num_virtual_channels).map(|vc|(j,vc)).collect();
						//}
						let amount=(if target_amount<0
						{
							up_current[dim]-up_next[dim]
						}
						else
						{
							up_next[dim]-up_current[dim]
						}+side)%side;
						if amount<=limit
						{
							if amount>best_amount
							{
								best_amount=amount;
								best=vec![j];
							}
							else if amount==best_amount
							{
								best.push(j);
							}
						}
					}
				}
			}
			if best.is_empty()
			{
				panic!("No links improving {} dimension\n",dim);
			}
			//let vcs: std::iter::Filter<_,_> =if be_random
			//{
			//	(0..num_virtual_channels).filter(|vc|!self.shortest_reserved_virtual_channels.contains(vc))
			//}
			//else
			//{
			//	(0..num_virtual_channels).filter(|vc|!self.randomized_reserved_virtual_channels.contains(vc))
			//};
			//return vcs.flat_map(|vc| best.iter().map(|p|(*p,vc)).collect::<Vec<(usize,usize)>>()).collect();
			if be_random
			{
				//XXX not worth to box the closure, right?
				let vcs=(0..num_virtual_channels).filter(|vc|!self.shortest_reserved_virtual_channels.contains(vc));
				//return vcs.flat_map(|vc| best.iter().map(|p|(*p,vc)).collect::<Vec<(usize,usize)>>()).collect();
				let r= vcs.flat_map(|vc| best.iter().map(|p|CandidateEgress::new(*p,vc)).collect::<Vec<_>>()).collect();
				return Ok(RoutingNextCandidates{candidates:r,idempotent:true})
			}
			else
			{
				let vcs=(0..num_virtual_channels).filter(|vc|!self.randomized_reserved_virtual_channels.contains(vc));
				//return vcs.flat_map(|vc| best.iter().map(|p|(*p,vc)).collect::<Vec<(usize,usize)>>()).collect();
				let r= vcs.flat_map(|vc| best.iter().map(|p|CandidateEgress::new(*p,vc)).collect::<Vec<_>>()).collect();
				return Ok(RoutingNextCandidates{candidates:r,idempotent:true})
			};
		}
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, _target_server:Option<usize>, rng: &mut StdRng)
	{
		//let (target_location,_link_class)=topology.server_neighbour(target_server);
		//let target_router=match target_location
		//{
		//	Location::RouterPort{router_index,router_port:_} =>router_index,
		//	_ => panic!("The server is not attached to a router"),
		//};
		//ValiantDOR needs cartesian data in the topology, which could be a dragonfly or whatever...
		let cartesian_data=topology.cartesian_data().expect("ValiantDOR requires a Cartesian topology");
		let up_current=cartesian_data.unpack(current_router);
		let mut up_target=cartesian_data.unpack(target_router);
		//let routing_record=(0..up_current.len()).map(|i|up_target[i] as i32-up_current[i] as i32).collect();//FIXME: torus
		let routing_record=topology.coordinated_routing_record(&up_current,&up_target,Some(rng));
		//println!("routing record from {} to {} is {:?}",current_router,target_router,routing_record);
		routing_info.borrow_mut().routing_record=Some(routing_record);
		let mut offset=0;
		let mut r=0;
		while offset<self.randomized.len()
		{
			//XXX Should we skip if current[dim]==target[dim]?
			let dim=self.randomized[offset];
			let side=cartesian_data.sides[dim];
		 	let t=rng.gen_range(0..side);
			up_target[dim]=t;
			let aux_rr=topology.coordinated_routing_record(&up_current,&up_target,Some(rng));
			r=aux_rr[dim];
			if r!=0
			{
				break;
			}
			offset+=1;
		}
		if offset<self.randomized.len()
		{
			routing_info.borrow_mut().selections=Some(vec![offset as i32,r]);
		}
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, _target_server:Option<usize>, rng: &mut StdRng)
	{
		//let dimension=current_port/2;
		//let delta=if current_port%2==0 { -1i32 } else { 1i32 };
		let cartesian_data=topology.cartesian_data().expect("ValiantDOR requires a Cartesian topology");
		if let (Location::RouterPort{router_index: previous_router, router_port:_},dimension)=topology.neighbour(current_router,current_port)
		{
			let up_current=cartesian_data.unpack(current_router);
			let up_previous=cartesian_data.unpack(previous_router);
			let side=cartesian_data.sides[dimension] as i32;
			let mut b_routing_info=routing_info.borrow_mut();
			match b_routing_info.routing_record
			{
				Some(ref mut rr) =>
				{
					let delta:i32=if rr[dimension]<0
					{
						(up_previous[dimension] as i32 - up_current[dimension] as i32 + side)%side
					}
					else
					{
						-((up_current[dimension] as i32 - up_previous[dimension] as i32 + side)%side)
					};
					rr[dimension]+=delta;
					// --- DEBUG vvv
					//let (target_location,_link_class)=topology.server_neighbour(target_server);
					//let target_router=match target_location
					//{
					//	Location::RouterPort{router_index,router_port:_} =>router_index,
					//	_ => panic!("The server is not attached to a router"),
					//};
					//let up_target=cartesian_data.unpack(target_router);
					//println!("new routing record. current_router={}({:?}, current_port={} previous_router={}({:?}), delta={}, rr={:?}, target_server={} target_router={}({:?})",current_router,up_current,current_port,previous_router,up_previous,delta,rr,target_server,target_router,up_target);
					// --- DEBUG ^^^
				},
				None => panic!("trying to update without routing_record"),
			};
			let sel = b_routing_info.selections.clone();
			match sel
			{
				Some(ref v) =>
				{
					let mut offset=v[0] as usize;
					let mut r=v[1];
					if dimension != self.randomized[offset]
					{
						panic!("Incorrect dimension while randomizing");
					}
					let delta:i32=if r<0
					{
						(up_previous[dimension] as i32 - up_current[dimension] as i32 + side)%side
					}
					else
					{
						-((up_current[dimension] as i32 - up_previous[dimension] as i32 + side)%side)
					};
					r+=delta;
					let target_router = if r!=0 { None } else
					{
						//let (target_location,_link_class)=topology.server_neighbour(target_server);
						//let target_router=match target_location
						//{
						//	Location::RouterPort{router_index,router_port:_} =>router_index,
						//	_ => panic!("The server is not attached to a router"),
						//};
						Some(target_router)
					};
					while r==0 && offset<self.randomized.len()-1
					{
						offset+=1;
						let dim=self.randomized[offset];
						//XXX Should we skip if current[dim]==target[dim]?
						let side=cartesian_data.sides[dim];
						let t=rng.gen_range(0..side);
						let mut up_target=cartesian_data.unpack(target_router.unwrap());
						up_target[dim]=t;
						let aux_rr=topology.coordinated_routing_record(&up_current,&up_target,Some(rng));
						r=aux_rr[dim];
					}
					if r==0
					{
						b_routing_info.selections=None;
						//remake routing record to ensure it is minimum
						let up_target=cartesian_data.unpack(target_router.unwrap());
						let routing_record=topology.coordinated_routing_record(&up_current,&up_target,Some(rng));
						b_routing_info.routing_record=Some(routing_record);
					}
					else
					{
						b_routing_info.selections=Some(vec![offset as i32,r]);
					};
				}
				None => (),
			};
		}
		else
		{
			panic!("!!");
		}
	}
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &mut StdRng)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>, _num_virtual_channels:usize, _rng:&mut StdRng)
	{
	}
	fn statistics(&self, _cycle:Time) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:Time)
	{
	}
}

impl ValiantDOR
{
	pub fn new(arg:RoutingBuilderArgument) -> ValiantDOR
	{
		let mut randomized=None;
		let mut shortest=None;
		let mut randomized_reserved_virtual_channels=None;
		let mut shortest_reserved_virtual_channels=None;
		//let mut servers_per_router=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="ValiantDOR"
			{
				panic!("A ValiantDOR must be created from a `ValiantDOR` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"randomized" => match value
					{
						&ConfigurationValue::Array(ref a) => randomized=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in randomized"),
						}).collect()),
						_ => panic!("bad value for randomized"),
					}
					"shortest" => match value
					{
						&ConfigurationValue::Array(ref a) => shortest=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in shortest"),
						}).collect()),
						_ => panic!("bad value for shortest"),
					}
					"randomized_reserved_virtual_channels" => match value
					{
						&ConfigurationValue::Array(ref a) => randomized_reserved_virtual_channels=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in randomized_reserved_virtual_channels"),
						}).collect()),
						_ => panic!("bad value for randomized_reserved_virtual_channels"),
					}
					"shortest_reserved_virtual_channels" => match value
					{
						&ConfigurationValue::Array(ref a) => shortest_reserved_virtual_channels=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in shortest_reserved_virtual_channels"),
						}).collect()),
						_ => panic!("bad value for shortest_reserved_virtual_channels"),
					}
					//"servers_per_router" => match value
					//{
					//	&ConfigurationValue::Number(f) => servers_per_router=Some(f as usize),
					//	_ => panic!("bad value for servers_per_router"),
					//}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in ValiantDOR",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a ValiantDOR from a non-Object");
		}
		let randomized=randomized.expect("There were no randomized");
		let shortest=shortest.expect("There were no shortest");
		let randomized_reserved_virtual_channels=randomized_reserved_virtual_channels.expect("There were no randomized_reserved_virtual_channels");
		let shortest_reserved_virtual_channels=shortest_reserved_virtual_channels.expect("There were no shortest_reserved_virtual_channels");
		ValiantDOR{
			randomized,
			shortest,
			randomized_reserved_virtual_channels,
			shortest_reserved_virtual_channels,
		}
	}
}


///The O1TTURN routing uses DOR order `[0,1]` for some virtual channels and order `[1,0]` for others.
///By default it reserves the channel 0 for `[0,1]` and the channel 1 for `[1,0]`.
#[derive(Debug)]
pub struct O1TURN
{
	/// Virtual channels reserved exclusively for the 0 before 1 DOR selection.
	/// Defaults to `[0]`
	reserved_virtual_channels_order01: Vec<usize>,
	/// Virtual channels reserved exclusively for the 1 before 0 DOR selection.
	/// Defaults to `[1]`
	reserved_virtual_channels_order10: Vec<usize>,
}

impl Routing for O1TURN
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, _target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, _rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//let routing_record=&routing_info.routing_record.expect("DOR requires a routing record");
		let routing_record=if let Some(ref rr)=routing_info.routing_record
		{
			rr
		}
		else
		{
			panic!("O1TURN requires a routing record");
		};
		if routing_record.len()!=2
		{
			panic!("O1TURN only works for bidimensional cartesian topologies");
		}
		let mut i=0;
		let s=routing_info.selections.as_ref().unwrap()[0] as usize;
		let order=match s
		{
			0 => [0,1],
			1 => [1,0],
			_ => panic!("Out of selection"),
		};
		while i<2 && routing_record[order[i]]==0
		{
			i+=1;
		}
		let forbidden_virtual_channels=match s
		{
			0 => &self.reserved_virtual_channels_order10,
			1 => &self.reserved_virtual_channels_order01,
			_ => unreachable!(),
		};
		let available_virtual_channels=(0..num_virtual_channels).filter(|vc|!forbidden_virtual_channels.contains(vc));
		if i==2
		{
			//To server
			let target_server = target_server.expect("target server was not given.");
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						//return vec![CandidateEgress::new(i,s)];
						let r= available_virtual_channels.map(|vc| CandidateEgress::new(i,vc)).collect();
						return Ok(RoutingNextCandidates{candidates:r,idempotent:true});
					}
				}
			}
			panic!("The server {} is not attached to this router ({}) but the routing record is {:?}",target_server,current_router,routing_record);
		}
		else
		{
			i=order[i];
			//Go in dimension i
			//WARNING: This assumes ports in a mesh-like configuration!
			let p=if routing_record[i]<0
			{
				2*i
			}
			else
			{
				2*i+1
			};
			//return vec![CandidateEgress::new(p,s)];
			let r= available_virtual_channels.map(|vc| CandidateEgress::new(p,vc)).collect();
			return Ok(RoutingNextCandidates{candidates:r,idempotent:true});
		}
	}
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, toology:&dyn Topology, current_router:usize, target_server:usize)
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, _target_server:Option<usize>, rng: &mut StdRng)
	{
		//let (target_location,_link_class)=topology.server_neighbour(target_server);
		//let target_router=match target_location
		//{
		//	Location::RouterPort{router_index,router_port:_} =>router_index,
		//	_ => panic!("The server is not attached to a router"),
		//};
		//O1TURN needs cartesian data in the topology, which could be a dragonfly or whatever...
		let cartesian_data=topology.cartesian_data().expect("O1TURN requires a Cartesian topology");
		let up_current=cartesian_data.unpack(current_router);
		let up_target=cartesian_data.unpack(target_router);
		//let routing_record=(0..up_current.len()).map(|i|up_target[i] as i32-up_current[i] as i32).collect();//FIXME: torus
		let routing_record=topology.coordinated_routing_record(&up_current,&up_target,Some(rng));
		//println!("routing record from {} to {} is {:?}",current_router,target_router,routing_record);
		routing_info.borrow_mut().routing_record=Some(routing_record);
		routing_info.borrow_mut().selections=Some(vec![{
			rng.gen_range(0..2)
		}]);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, current_port:usize, _target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		let dimension=current_port/2;
		let delta=if current_port%2==0 { -1i32 } else { 1i32 };
		match routing_info.borrow_mut().routing_record
		{
			Some(ref mut rr) =>
			{
				rr[dimension]+=delta;
				//println!("new routing record at ({},{}) is {:?}",current_router,current_port,rr);
			},
			None => panic!("trying to update without routing_record"),
		};
	}
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &mut StdRng)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>, _num_virtual_channels:usize, _rng:&mut StdRng)
	{
	}
	fn statistics(&self, _cycle:Time) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:Time)
	{
	}
}

impl O1TURN
{
	pub fn new(arg:RoutingBuilderArgument) -> O1TURN
	{
		//let mut order=None;
		//let mut servers_per_router=None;
		let mut reserved_virtual_channels_order01: Option<Vec<usize>> = None;
		let mut reserved_virtual_channels_order10: Option<Vec<usize>> = None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="O1TURN"
			{
				panic!("A O1TURN must be created from a `O1TURN` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					//"order" => match value
					//{
					//	&ConfigurationValue::Array(ref a) => order=Some(a.iter().map(|v|match v{
					//		&ConfigurationValue::Number(f) => f as usize,
					//		_ => panic!("bad value in order"),
					//	}).collect()),
					//	_ => panic!("bad value for order"),
					//}
					"reserved_virtual_channels_order01" => match value
					{
						&ConfigurationValue::Array(ref a) => reserved_virtual_channels_order01=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in reserved_virtual_channels_order01"),
						}).collect()),
						_ => panic!("bad value for reserved_virtual_channels_order01"),
					}
					"reserved_virtual_channels_order10" => match value
					{
						&ConfigurationValue::Array(ref a) => reserved_virtual_channels_order10=Some(a.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in reserved_virtual_channels_order10"),
						}).collect()),
						_ => panic!("bad value for reserved_virtual_channels_order10"),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in O1TURN",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a O1TURN from a non-Object");
		}
		//let order=order.expect("There were no order");
		let reserved_virtual_channels_order01=reserved_virtual_channels_order01.unwrap_or_else(||vec![0]);
		let reserved_virtual_channels_order10=reserved_virtual_channels_order10.unwrap_or_else(||vec![1]);
		O1TURN{
			reserved_virtual_channels_order01,
			reserved_virtual_channels_order10,
		}
	}
}



///A policy for the `SumRouting` about how to select among the two `Routing`s.
#[derive(Debug)]
pub enum GeneralTurnPolicy
{
	///Random at source.
	Random,
	///Only injected in an order which contains an unaligned dimension.
	UnalignedDimension,
	///Adaptive without discarding other compatible options.
	Adaptive,
	///Decision taken at source, but once taken, it is kept.
	AdaptiveSource,
}

pub fn new_general_turn_policy(cv: &ConfigurationValue) -> GeneralTurnPolicy
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=cv
	{
		match cv_name.as_ref()
		{
			"Random" => GeneralTurnPolicy::Random,
			"UnalignedDimension" => GeneralTurnPolicy::UnalignedDimension,
			"AdaptiveSource" => GeneralTurnPolicy::AdaptiveSource,
			_ => panic!("Unknown generalturn_policy {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create a GeneralTurnPolicy from a non-Object");
	}
}


///Routing adapted from "Near-optimal worst-case throughput routing for two-dimensional mesh networks" by Daeho Seo, et al.
/// It asigns a dimension order to each virtual channel.
#[derive(Debug)]
pub struct GENERALTURN
{
	orders: Vec<Vec<usize>>,
	virtual_channels: Vec<Vec<usize>>,
	policy:GeneralTurnPolicy,
}

impl Routing for GENERALTURN
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, _rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
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
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						//return (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
					}
				}
			}
			unreachable!();
		}

		let cartesian_data=topology.cartesian_data().expect("GENERALTURN requires a Cartesian topology");
		//let routing_record=&routing_info.routing_record.expect("DOR requires a routing record");
		let routing_record=if let Some(ref rr)=routing_info.routing_record
		{
			rr
		}
		else
		{
			panic!("GENERALTURN requires a routing record");
		};

		/*if routing_record.len()!=2
		{
			panic!("GENERALTURN only works for bidimensional cartesian topologies");
		}*/

		//let s=routing_info.selections.as_ref().unwrap()[0] as usize;
		let s=routing_info.selections.clone().unwrap();
		let mut r = vec![];

		for order_index in s
		{
			let order_index = order_index as usize;
			let order = &self.orders[order_index];
			//let order=self.orders[s];
		    let mut i=0;
			while routing_record[order[i]]==0
			{
				i+=1;
			}
			let available_virtual_channels= &self.virtual_channels[order_index];
			//let available_virtual_channels=(0..num_virtual_channels).filter(|vc|!forbidden_virtual_channels.contains(vc));

			i=order[i];
			//Go in dimension i
			//WARNING: This assumes ports in a mesh-like configuration!


			let offset = if routing_record[i] > 0i32 { routing_record[i] as usize } else { (routing_record[i] + cartesian_data.sides[i] as i32) as usize };
			let p = (cartesian_data.sides[i]-1) * i + offset -1;
			/*let p=if routing_record[i]<0
			{
				cartesian_data.sides[i] * i +
			}
			else
			{
				2*i+1
			};*/


			//return vec![CandidateEgress::new(p,s)]; .clone()
			//CandidateEgress{virtual_channel:avc0[candidate.virtual_channel],label:candidate.label+el0,annotation:Some(RoutingAnnotation{values:vec![0],meta:vec![candidate.annotation]}),..candidate}
			let candidates:Vec<CandidateEgress> = available_virtual_channels.into_iter()
				.map(|vc|
						 CandidateEgress{port: p, virtual_channel:*vc,label:0,annotation:Some(RoutingAnnotation{values:vec![0], meta: vec![] }),..Default::default()}
					 )
				.collect();
			r.extend(candidates.into_iter());
		}

		return Ok(RoutingNextCandidates{candidates:r,idempotent:true});
	}
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, toology:&dyn Topology, current_router:usize, target_server:usize)
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, _target_server:Option<usize>, rng: &mut StdRng)
	{
		//let (target_location,_link_class)=topology.server_neighbour(target_server);
		//let target_router=match target_location
		//{
		//	Location::RouterPort{router_index,router_port:_} =>router_index,
		//	_ => panic!("The server is not attached to a router"),
		//};
		//GENERALTURN needs cartesian data in the topology, which could be a dragonfly or whatever...
		let cartesian_data=topology.cartesian_data().expect("GENERALTURN requires a Cartesian topology");
		let up_current=cartesian_data.unpack(current_router);
		let up_target=cartesian_data.unpack(target_router);
		//let routing_record=(0..up_current.len()).map(|i|up_target[i] as i32-up_current[i] as i32).collect();//FIXME: torus
		let routing_record=topology.coordinated_routing_record(&up_current,&up_target,Some(rng));
		//println!("routing record from {} to {} is {:?}",current_router,target_router,routing_record);
		routing_info.borrow_mut().routing_record=Some(routing_record.clone());

		let all:Vec<i32> = match self.policy
		{
			GeneralTurnPolicy::Random => vec![ rng.gen_range(0..self.orders.len()) as i32],
			GeneralTurnPolicy::UnalignedDimension => (0..self.orders.len()).filter(|i|  routing_record[ self.orders[*i][0] ] != 0 ).map(|a| a as i32).collect::<Vec<i32>>(),
			GeneralTurnPolicy::AdaptiveSource =>  (0..self.orders.len()).map(|a| a as i32).collect::<Vec<i32>>(),
			GeneralTurnPolicy::Adaptive => unimplemented!(),

		};
		routing_info.borrow_mut().selections=Some(all);

		//routing_info.borrow_mut().selections=Some(vec![{
		//	rng.gen_range(0..2)
		//}]
		//);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, _current_router:usize, current_port:usize, _target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		let cartesian_data=topology.cartesian_data().expect("GENERALTURN requires a Cartesian topology");
		let dimension=current_port/(cartesian_data.sides[0] -1 );
		//let delta=if current_port%2==0 { -1i32 } else { 1i32 };
		match routing_info.borrow_mut().routing_record
		{
			Some(ref mut rr) =>
				{
					rr[dimension]=0; // as its minimal routing in hamming diameter is 1 inside the dimension
					//println!("new routing record at ({},{}) is {:?}",current_router,current_port,rr);
				},
			None => panic!("trying to update without routing_record"),
		};


		//let hops = routing_info.borrow_mut().hops - 1;
		let sel = routing_info.borrow_mut().selections.as_ref().unwrap().clone();

		/*routing_info.borrow_mut().selections=Some(
				sel.into_iter().into_iter().filter(|a| sel.count(a) == 2).collect::<Vec<i32>>()
				//sel.into_iter().filter(|order_index| self.orders[*order_index as usize][hops] == dimension).collect::<Vec<i32>>()
		);*/

		if sel.len() > 1
		{
			routing_info.borrow_mut().selections=Some( vec![*sel.last().unwrap()]);
		}

	}
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &mut StdRng)
	{
	}
	fn performed_request(&self, requested:&CandidateEgress, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>, _num_virtual_channels:usize, _rng:&mut StdRng)
	{

		let mut bri=routing_info.borrow_mut();
		//bri.meta=Some(vec![RefCell::new(RoutingInfo::new()),RefCell::new(RoutingInfo::new())]);
		let mut selections = bri.selections.clone().unwrap().into_iter().unique().collect::<Vec<i32>>();

		if selections.len()>1 //FIXME: THIS IS SPAGHETTI CODE, IM SORRY. The order index in vec selections saved 2 times is the order to follow.
		{
			let &CandidateEgress{ref annotation,..} = requested;
			if let Some(annotation) = annotation.as_ref()
			{
				let s = annotation.values[0];
				selections.push(s);
				bri.selections = Some(selections);

			}
		}
	}
	fn statistics(&self, _cycle:Time) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:Time)
	{
	}
}

impl GENERALTURN
{
	pub fn new(arg:RoutingBuilderArgument) -> GENERALTURN
	{
		//let mut order=None;
		//let mut servers_per_router=None;
		let mut orders: Option<Vec<Vec<usize>>> = None;
		let mut virtual_channels: Option<Vec<Vec<usize>>> = None;
		let mut policy: Option<GeneralTurnPolicy> = None;

		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="GeneralTurn"
			{
				panic!("A GeneralTurn must be created from a `GeneralTurn` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					//"order" => match value
					//{
					//	&ConfigurationValue::Array(ref a) => order=Some(a.iter().map(|v|match v{
					//		&ConfigurationValue::Number(f) => f as usize,
					//		_ => panic!("bad value in order"),
					//	}).collect()),
					//	_ => panic!("bad value for order"),
					//}
					"orders" => match value
					{
						&ConfigurationValue::Array(ref a) => orders=Some(a.iter().map(|v|match v
                        {
                            &ConfigurationValue::Array(ref a) => a.iter().map(|v|match v
                            {
                                &ConfigurationValue::Number(f) => f as usize,
                                _ => panic!("bad value in orders"),
                            }).collect(),
                            _ => panic!("bad value in orders"),
                        }).collect()),
						_ => panic!("bad value for orders"),
					}
					"virtual_channels" => match value
                    {
                        &ConfigurationValue::Array(ref a) => virtual_channels=Some(a.iter().map(|v|match v
                        {
                            &ConfigurationValue::Array(ref a) => a.iter().map(|v|match v
                            {
                                &ConfigurationValue::Number(f) => f as usize,
                                _ => panic!("bad value in virtual_channels"),
                            }).collect(),
                            _ => panic!("bad value in virtual_channels"),
                        }).collect()),
                        _ => panic!("bad value for virtual_channels"),
                    }
					"policy" => policy=Some(new_general_turn_policy(value)),
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in GENERALTURN",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a GENERALTURN from a non-Object");
		}

		let orders=orders.expect("There were no orders");
		let virtual_channels=virtual_channels.expect("There were no virtual_channels");
		let policy=policy.expect("There were no policy");

		GENERALTURN{
            orders,
            virtual_channels,
            policy,
        }
	}
}




/// Routing part of the Omni-dimensional Weighted Adaptive Routing of Nic McDonald et al.
/// Stores `RoutingInfo.selections=Some(vec![available_deroutes])`.
/// Only paths of currently unaligned dimensions are valid. Otherwise dimensions are ignored.
#[derive(Debug)]
pub struct OmniDimensionalDeroute
{
	///Maximum number of non-shortest (deroutes) hops to make.
	allowed_deroutes: usize,
	///To mark non-shortest options with label=1.
	include_labels: bool,
}

impl Routing for OmniDimensionalDeroute
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, _rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//let (target_location,_link_class)=topology.server_neighbour(target_server);
		//let target_router=match target_location
		//{
		//	Location::RouterPort{router_index,router_port:_} =>router_index,
		//	_ => panic!("The server is not attached to a router"),
		//};
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
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						//return (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
					}
				}
			}
			unreachable!();
		}
		let available_deroutes=routing_info.selections.as_ref().unwrap()[0] as usize;
		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);
		if available_deroutes==0
		{
			//Go minimally.
			for i in 0..num_ports
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::RouterPort{router_index,router_port:_},_link_class)=topology.neighbour(current_router,i)
				{
					if distance-1==topology.distance(router_index,target_router)
					{
						//r.extend((0..num_virtual_channels).map(|vc|(i,vc)));
						r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)));
					}
				}
			}
		}
		else
		{
			//Include any unaligned.
			let cartesian_data=topology.cartesian_data().expect("OmniDimensionalDeroute requires a Cartesian topology");
			let up_current=cartesian_data.unpack(current_router);
			let up_target=cartesian_data.unpack(target_router);
			for i in 0..num_ports
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::RouterPort{router_index,router_port:_},_link_class)=topology.neighbour(current_router,i)
				{
					let up_next=cartesian_data.unpack(router_index);
					let mut good=true;
					for j in 0..up_next.len()
					{
						if up_current[j]==up_target[j] && up_current[j]!=up_next[j]
						{
							good=false;
							break;
						}
					}
					if good
					{
						//r.extend((0..num_virtual_channels).map(|vc|(i,vc)));
						if self.include_labels && topology.distance(router_index,target_router)>=distance
						{
							r.extend((0..num_virtual_channels).map(|vc|CandidateEgress{port:i,virtual_channel:vc,label:1,..Default::default()}));
						}
						else
						{
							r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)));
						}
					}
				}
			}
		}
		Ok(RoutingNextCandidates{candidates:r,idempotent:true})
	}
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, toology:&dyn Topology, current_router:usize, target_server:usize)
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		routing_info.borrow_mut().selections=Some(vec![self.allowed_deroutes as i32]);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		//let cartesian_data=topology.cartesian_data().expect("OmniDimensionalDeroute requires a Cartesian topology");
		if let (Location::RouterPort{router_index: previous_router,router_port:_},_link_class)=topology.neighbour(current_router,current_port)
		{
			//let up_current=cartesian_data.unpack(current_router);
			//let up_previous=cartesian_data.unpack(previous_router);
			//let (target_location,_link_class)=topology.server_neighbour(target_server);
			//let target_router=match target_location
			//{
			//	Location::RouterPort{router_index,router_port:_} =>router_index,
			//	_ => panic!("The server is not attached to a router"),
			//};
			//let up_target=cartesian_data.unpack(target_router);
			if topology.distance(previous_router,target_router)!=1+topology.distance(current_router,target_router)
			{
				match routing_info.borrow_mut().selections
				{
					Some(ref mut v) =>
					{
						let available_deroutes=v[0];
						if available_deroutes==0
						{
							panic!("We should have not done this deroute.");
						}
						v[0]=available_deroutes-1;
					}
					None => panic!("available deroutes not initialized"),
				};
			}
		}
	}
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &mut StdRng)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>, _num_virtual_channels:usize, _rng:&mut StdRng)
	{
	}
	fn statistics(&self, _cycle:Time) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:Time)
	{
	}
}

impl OmniDimensionalDeroute
{
	pub fn new(arg:RoutingBuilderArgument) -> OmniDimensionalDeroute
	{
		let mut allowed_deroutes=None;
		let mut include_labels=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="OmniDimensionalDeroute"
			{
				panic!("A OmniDimensionalDeroute must be created from a `OmniDimensionalDeroute` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"allowed_deroutes" => match value
					{
						&ConfigurationValue::Number(f) => allowed_deroutes=Some(f as usize),
						_ => panic!("bad value for allowed_deroutes"),
					}
					"include_labels" => match value
					{
						&ConfigurationValue::True => include_labels=Some(true),
						&ConfigurationValue::False => include_labels=Some(false),
						_ => panic!("bad value for include_labels"),
					}
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in OmniDimensionalDeroute",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a OmniDimensionalDeroute from a non-Object");
		}
		let allowed_deroutes=allowed_deroutes.expect("There were no allowed_deroutes");
		let include_labels=include_labels.expect("There were no include_labels");
		OmniDimensionalDeroute{
			allowed_deroutes,
			include_labels,
		}
	}
}



/// Routing part of the Omni-dimensional Weighted Adaptive Routing of Nic McDonald et al.
/// It's the Omnidimensional cheap version, it only needs 2vc to be deadlock free.
/// Traverses the dimensions in order, allowing one deroute per dimension.
/// Candidates are always marked with a label to favour VC policies, following the next logic:
/// 0: Minimal routing (VC 0)
/// 1: Non-minimal routing (VC 0)
/// 2: Forced minimal routing (VC 1)
#[derive(Debug)]
pub struct DimWAR
{
	order: Vec<usize>,
}

impl Routing for DimWAR
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, _rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		/*let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};*/
		let distance=topology.distance(current_router,target_router);

		if distance==0
		{
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server.expect("Server here!")
					{
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						//return (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
					}
				}
			}
			unreachable!();
		}

		let miss_dim = routing_info.selections.as_ref().unwrap();
		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);

		let cartesian_data=topology.cartesian_data().expect("OmniDimensionalDeroute requires a Cartesian topology");
		//let cartesian_sides = cartesian_data.sides;
		let up_current=cartesian_data.unpack(current_router);
		let up_target=cartesian_data.unpack(target_router);

		let mut dimension_exit = None;
		//for j in 0..up_target.len()
		for j in &self.order
		{
			if up_current[*j]!=up_target[*j]
			{
				dimension_exit = Some(*j);
				break;
			}
		}
		let dimension_exit = dimension_exit.expect("Next DOR dimension should exist");

		let mut port_offset = 0;
		for j in 0..dimension_exit
		{
			port_offset += cartesian_data.sides[j]-1;
		}

		for i in port_offset..(port_offset + cartesian_data.sides[dimension_exit] -1)
		{
			//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
			if let (Location::RouterPort{router_index,router_port:_}, link_class)=topology.neighbour(current_router,i)
			{
				//let up_next=cartesian_data.unpack(router_index);
				// panic if link_class do not match the exit dimension
				if link_class != dimension_exit
				{
					panic!("The port is not pointing in the right direction, link_class: {} exit_dimension {}", link_class, dimension_exit);
				}
				//r.extend((0..num_virtual_channels).map(|vc|(i,vc)));
				if miss_dim[dimension_exit] == 1 //can missroutte topology.distance(router_index,target_router) >= distance
				{
					if topology.distance(router_index,target_router) < distance
					{
						r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc))); //MIN

					}else{

						r.extend((0..num_virtual_channels).map(|vc|CandidateEgress{port:i,virtual_channel:vc,label: 1,..Default::default()})); //NON MINIMAL
					}

				}
				else if topology.distance(router_index,target_router) < distance
				{
					//r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc))); //MIN
					r.extend((0..num_virtual_channels).map(|vc|CandidateEgress{port:i,virtual_channel:vc,label: 2,..Default::default()})); //FORCED MINIMAL ROUTING
				}
			}
		}

		Ok(RoutingNextCandidates{candidates:r,idempotent:true})
	}
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, toology:&dyn Topology, current_router:usize, target_server:usize)
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		/*let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};*/

		let cartesian_data=topology.cartesian_data().expect("OmniDimensionalDeroute requires a Cartesian topology");
		let mut missrouting_vector = vec![0; cartesian_data.sides.len()];

		let up_current=cartesian_data.unpack(current_router);
		let up_target=cartesian_data.unpack(target_router);

		for component in 0..up_current.len()
		{
			if up_current[component] != up_target[component]
			{
				missrouting_vector[component] = 1i32;
			}
		}

		routing_info.borrow_mut().selections=Some(missrouting_vector);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, _target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)

	{
		if let (Location::RouterPort{router_index: previous_router,router_port:_},_link_class)=topology.neighbour(current_router,current_port)
		{

			let cartesian_data=topology.cartesian_data().expect("OmniDimensionalDeroute requires a Cartesian topology");
			let up_current=cartesian_data.unpack(current_router);
			let up_previous=cartesian_data.unpack(previous_router);
			let mut exit_dimension = None;

			for j in 0..up_current.len()
			{
				if up_current[j] != up_previous[j]
				{
					exit_dimension = Some(j);
				}
			}

			let exit_dimension = exit_dimension.expect("The port doesnt reach any other router");

			match routing_info.borrow_mut().selections
			{
				Some(ref mut v) =>
					{
						//if v[exit_dimension]==0
						//{
						//	panic!("We should have not done this deroute.");
						//}
						//¿Should we check that the hop is minimal if it v[exit_dim] == 0?
						v[exit_dimension]= 0;
					}
				None => panic!("available deroutes not initialized"),
			};
		}
	}
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &mut StdRng)
	{

	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>,  _num_virtual_channels:usize, _rng: &mut StdRng)
	{
	}
	fn statistics(&self, _cycle:Time) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:Time)
	{
	}
}

impl DimWAR
{
	pub fn new(arg:RoutingBuilderArgument) -> DimWAR
	{
		let mut order= None;

		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="DimWAR"
			{
				panic!("A DimWAR must be created from a `DimWAR` object not `{}`",cv_name);
			}
			for &(ref name,ref _value) in cv_pairs
			{
				//match name.as_ref()
				match AsRef::<str>::as_ref(&name)
				{
					"order" => order = match _value
					{
						&ConfigurationValue::Array(ref v) => Some(v.iter().map(|cv|match cv
						{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value for order"),
						}).collect()),
						_ => panic!("bad value for order"),
					},
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in OmniDimensionalDeroute",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a OmniDimensionalDeroute from a non-Object");
		}
		let order=order.expect("There were no order");

		DimWAR{
			order
		}
	}
}


/**
This is an adapted Valiant version for the Hamming topology, suitable for source adaptive routings, as UGAL.
It removes intermediate switches aligned with source or destination selected dimensions.
It also can forbid to missroute in a dimension if the source is already aligned.
See Valiant, L. G. (1982). A scheme for fast parallel communication. SIAM journal on computing, 11(2), 350-361.

```ignore
Valiant4Hamming{
	first: Shortest,
	second: Shortest,
	first_reserved_virtual_channels: [0],//optional parameter, defaults to empty. Reserves some VCs to be used only in the first stage
	second_reserved_virtual_channels: [1,2],//optional, defaults to empty. Reserves some VCs to be used only in the second stage.
	remove_target_dimensions_aligment:[[0],[1]], //remove intermediate aligned with the target in the 0 and 1 dimensions
	remove_source_dimensions_aligment:[[0],[1]]  //remove intermediate aligned with the source in the 0 and 1 dimensions
	allow_unaligned: false, //To go through unaligned dimensions
	legend_name: "Using Valiant4Hamming scheme, shortest to intermediate and shortest to destination",
}
```
 **/
#[derive(Debug)]
pub struct Valiant4Hamming
{
	first: Box<dyn Routing>,
	second: Box<dyn Routing>,
	//pattern to select intermideate nodes
	pattern:Box<dyn Pattern>,
	first_reserved_virtual_channels: Vec<usize>,
	second_reserved_virtual_channels: Vec<usize>,
	remove_target_dimensions_aligment: Vec<Vec<usize>>,
	remove_source_dimensions_aligment: Vec<Vec<usize>>,
	allow_unaligned: bool,
}

impl Routing for Valiant4Hamming
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

	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, _target_server:Option<usize>, rng: &mut StdRng)
	{
		let port = topology.degree(current_router);
		let (server_source_location,_link_class) = topology.neighbour(current_router, port);
		let source_server=match server_source_location
		{
			Location::ServerPort(server) =>server,
			_ => panic!("The server is not attached to a router"),
		};

		let cartesian_data = topology.cartesian_data().expect("Should be a cartesian data");//.expect("something").unpack(current_router)[1] ==

		let src_coord = cartesian_data.unpack(current_router);
		let trg_coord = cartesian_data.unpack(target_router);

		//let middle_server = self.pattern.get_destination(source_server,topology, rng);
		//let (middle_location,_link_class)=topology.server_neighbour(middle_server);
		let mut middle_router;
		// =match middle_location
		// {
		// 	Location::RouterPort{router_index,router_port:_} =>router_index,
		// 	_ => panic!("The server is not attached to a router"),
		// };

		let mut middle_coord= vec![]; // = cartesian_data.unpack(middle_router);
		let mut not_valid_middle = true;

		while not_valid_middle
		{
			not_valid_middle = false;
			let middle_server = self.pattern.get_destination(source_server,topology, rng);
			let (middle_location,_link_class)=topology.server_neighbour(middle_server);
			middle_router=match middle_location
			{
				Location::RouterPort{router_index,router_port:_} =>router_index,
				_ => panic!("The server is not attached to a router"),
			};
			middle_coord = cartesian_data.unpack(middle_router);

			if middle_router ==  current_router || middle_router == target_router
			{
				not_valid_middle=true;
			}

			for i in 0..self.remove_target_dimensions_aligment.len()
			{
				let dimensions = self.remove_target_dimensions_aligment[i].clone();
				let mut differ = false;
				for z in 0..dimensions.len()
				{
					if trg_coord[dimensions[z]] != middle_coord[dimensions[z]]
					{
						differ = true;
					}
				}
				if !differ
				{
					not_valid_middle = true;
					break; //the inner loop
				}
			}

			for i in 0..self.remove_source_dimensions_aligment.len()
			{
				let dimensions = self.remove_source_dimensions_aligment[i].clone();
				let mut differ = false;
				for z in 0..dimensions.len()
				{
					if src_coord[dimensions[z]] != middle_coord[dimensions[z]]
					{
						differ = true;
					}
				}
				if !differ
				{
					not_valid_middle = true;
					break; //the inner loop
				}
			}
		}

		if !self.allow_unaligned
		{
			for i in 0..src_coord.len()
			{
				if src_coord[i] == trg_coord[i]
				{
					middle_coord[i] = trg_coord[i];
				}
			}
		}
		middle_router = cartesian_data.pack(&middle_coord);


		let mut bri=routing_info.borrow_mut();
		bri.visited_routers=Some(vec![current_router]);
		bri.meta=Some(vec![RefCell::new(RoutingInfo::new()),RefCell::new(RoutingInfo::new())]);

		bri.selections=Some(vec![middle_router as i32]);
		self.first.initialize_routing_info(&bri.meta.as_ref().unwrap()[0],topology,current_router,middle_router,None,rng)

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
					let at_middle = current_router == middle;

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

impl Valiant4Hamming
{
	pub fn new(arg: RoutingBuilderArgument) -> Valiant4Hamming
	{
		//let mut order=None;
		//let mut servers_per_router=None;
		let mut first=None;
		let mut second=None;
		let mut pattern: Box<dyn Pattern> = Box::new(UniformPattern::uniform_pattern(true)); //pattern to intermideate node
		let mut first_reserved_virtual_channels=vec![];
		let mut second_reserved_virtual_channels=vec![];
		let mut remove_target_dimensions_aligment = vec![];
		let mut remove_source_dimensions_aligment = vec![];
		let mut allow_unaligned = true;

		match_object_panic!(arg.cv,"Valiant4Hamming",value,
			"first" => first=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"second" => second=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
		    "pattern" => pattern= Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})).expect("pattern not valid for Valiant4Hamming"),
			"first_reserved_virtual_channels" => first_reserved_virtual_channels=value.
				as_array().expect("bad value for first_reserved_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in first_reserved_virtual_channels") as usize).collect(),
			"second_reserved_virtual_channels" => second_reserved_virtual_channels=value.
				as_array().expect("bad value for second_reserved_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in second_reserved_virtual_channels") as usize).collect(),
			"remove_target_dimensions_aligment" => remove_target_dimensions_aligment=value.
				as_array().expect("bad value for remove_dimension_aligment").iter().map(|v|v.as_array()
				.expect("bad value in remove_dimension_aligment").iter().map(|v|v.as_f64().expect("bad value in remove_dimension_aligment") as usize).collect()).collect(),
			"remove_source_dimensions_aligment" => remove_source_dimensions_aligment=value.
				as_array().expect("bad value for remove_source_dimensions_aligment").iter()
			.map(|v|v.as_array().expect(" bad value in remove_source_dimensions_aligment").iter().map(|v|v.as_f64().expect("bad value in remove_source_dimensions_aligment") as usize).collect()).collect(),
			"allow_unaligned" => allow_unaligned=value.as_bool().expect("bad value for allow_unaligned"),

		);
		let first=first.expect("There were no first");
		let second=second.expect("There were no second");

		Valiant4Hamming{
			first,
			second,
			pattern,
			first_reserved_virtual_channels,
			second_reserved_virtual_channels,
			remove_target_dimensions_aligment,
			remove_source_dimensions_aligment,
			allow_unaligned,
		}
	}
}


/**
	Non-minimal routing of Clos-AD routing from the Flattened Butterfly paper.
	Similar to Valiant, it missroutes to a switch in a dimension with the queue more empty.

```ignore
AdaptiveValiantClos{
	order:[0,1,2],
	first_reserved_virtual_channels: [0],
	second_reserved_virtual_channels: [1],
}
```
 **/
#[derive(Debug)]
pub struct AdaptiveValiantClos
{
	first: Box<dyn Routing>,
	second: Box<dyn Routing>,
	///Whether to avoid selecting routers without attached servers. This helps to apply it to indirect networks.
	// selection_exclude_indirect_routers: bool,
	//pattern to select intermideate nodes
	first_reserved_virtual_channels: Vec<usize>,
	second_reserved_virtual_channels: Vec<usize>,
}

impl Routing for AdaptiveValiantClos
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		/*let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};*/
		let distance=topology.distance(current_router,target_router);
		if distance==0
		{
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server.expect("Server here!")
					{
						//return (0..num_virtual_channels).map(|vc|(i,vc)).collect();
						//return (0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect();
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
					}
				}
			}
			unreachable!();
		}
		let cartesian_data = topology.cartesian_data().expect("Cartesian topology");
		let selections=routing_info.selections.as_ref().unwrap();
		let meta=routing_info.meta.as_ref().unwrap();
		let mut dimension_exit:i32 = -1;

		for i in 0..selections.len()
		{
			if selections[i] == 1
			{
				dimension_exit = i as i32;
				break;
			}
		}
		let mut r = vec![];
		if dimension_exit > -1 // GO miss
		{
			let dimension_exit = dimension_exit as usize;

			let mut port_offset = 0;
			for j in 0..dimension_exit
			{
				port_offset += cartesian_data.sides[j]-1;
			}

			for i in port_offset..(port_offset + cartesian_data.sides[dimension_exit] -1)
			{
				if let (Location::RouterPort{router_index: _,router_port:_},link_class)=topology.neighbour(current_router,i)
				{
					if link_class != dimension_exit
					{
						panic!("The port is not pointing in the right direction, link_class: {} exit_dimension {}", link_class, dimension_exit);
					}

					r.extend((self.first_reserved_virtual_channels.clone().into_iter()).map(|vc|CandidateEgress::new(i,vc)));
				}
			}

			Ok(RoutingNextCandidates{candidates:r,idempotent:true})
		}else{ //GO to destination

			let base=self.second.next(&meta[1].borrow(),topology,current_router,target_router, target_server,num_virtual_channels,rng)?;
			let idempotent = base.idempotent;
			r=base.into_iter().filter_map(|egress|
				{
					if !self.first_reserved_virtual_channels.contains(&egress.virtual_channel) && self.second_reserved_virtual_channels.contains(&egress.virtual_channel)
					{
						Some(egress)
					}else{
						None
					}
				}).collect();
			Ok(RoutingNextCandidates{candidates:r,idempotent})
		}
	}

	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{

		let port = topology.degree(current_router);
		let (server_source_location,_link_class) = topology.neighbour(current_router, port);
		let _source_server=match server_source_location
		{
			Location::ServerPort(server) =>server,
			_ => panic!("The server is not attached to a router"),
		};

		let cartesian_topology = topology.cartesian_data().expect("Cartesian topology");
		let mut dimension_deroute = vec![0; cartesian_topology.sides.len()];
		let mut deroutes = 0;
		let current_coord = cartesian_topology.unpack(current_router);
		let target_coord = cartesian_topology.unpack(target_router);
		for i in 0..cartesian_topology.sides.len()
		{
			// dimension deroute is 1 if random is 0
			if current_coord[i] != target_coord[i]
			{
				dimension_deroute[i] = 1;
				deroutes+=1;
			}
		}

		let mut bri=routing_info.borrow_mut();
		bri.visited_routers=Some(vec![current_router]);
		bri.meta=Some(vec![RefCell::new(RoutingInfo::new()),RefCell::new(RoutingInfo::new())]);

		//check if dimension_deroute has any 1 in it filtering the list into_iter
		if deroutes == 0
		{
			self.second.initialize_routing_info(&bri.meta.as_ref().unwrap()[1],topology,current_router,target_router, target_server,rng);
		}

		bri.selections=Some(dimension_deroute);

	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		if let (Location::RouterPort{router_index: previous_router,router_port:_},_link_class)=topology.neighbour(current_router,current_port)
		{
			let mut bri = routing_info.borrow_mut();
			let _hops = bri.hops;
			// let _middle = bri.selections.as_ref().map(|s| s[0] as usize);

			let cartesian_data = topology.cartesian_data().expect("OmniDimensionalDeroute requires a Cartesian topology");
			let up_current = cartesian_data.unpack(current_router);
			let up_previous = cartesian_data.unpack(previous_router);
			let mut exit_dimension = None;

			for j in 0..up_current.len()
			{
				if up_current[j] != up_previous[j]
				{
					exit_dimension = Some(j);
				}
			}

			let exit_dimension = exit_dimension.expect("there should be a usize here");
			match bri.selections
			{
				Some(ref mut v) =>
					{
						v[exit_dimension] = 0;
					}
				None => panic!("selections not initialized"),
			};

			match bri.visited_routers
			{
				Some(ref mut v) =>
					{
						v.push(current_router);
					}
				None => panic!("visited_routers not initialized"),
			};

			let deroutes = bri.selections.clone().expect("List of missrouting").into_iter().filter(|&x| x == 1i32).collect::<Vec<_>>().len();
			if deroutes == 0
			{
				self.second.initialize_routing_info(&bri.meta.as_ref().unwrap()[1], topology, current_router, target_router, target_server, rng);
			}
		}
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.first.initialize(topology,rng);
		self.second.initialize(topology,rng);

	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>,  _num_virtual_channels:usize, _rng: &mut StdRng)
	{
		//TODO: recurse over routings
	}
}

impl AdaptiveValiantClos
{
	pub fn new(arg: RoutingBuilderArgument) -> AdaptiveValiantClos
	{
		//let mut order=None;
		//let mut servers_per_router=None;
		let mut order=None;
		// let mut selection_exclude_indirect_routers=true; //this was false...
		let mut first_reserved_virtual_channels=vec![];
		let mut second_reserved_virtual_channels=vec![];
		match_object_panic!(arg.cv,"AdaptiveValiantClos",value,
			// "first" => first=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			// "second" => second=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"order" => order=Some(value.as_array().expect("bad value for order").iter().map(|v|v.as_f64().expect("bad value in order") as usize).collect()),
			// "selection_exclude_indirect_routers" => selection_exclude_indirect_routers = value.as_bool().expect("bad value for selection_exclude_indirect_routers"),
			"first_reserved_virtual_channels" => first_reserved_virtual_channels=value.
				as_array().expect("bad value for first_reserved_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in first_reserved_virtual_channels") as usize).collect(),
			"second_reserved_virtual_channels" => second_reserved_virtual_channels=value.
				as_array().expect("bad value for second_reserved_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in second_reserved_virtual_channels") as usize).collect(),
		);

		let order:Vec<usize>= order.expect("There were no order");
		let first= ConfigurationValue::Object("DOR".to_string(), vec![("order".to_string(), ConfigurationValue::Array( order.iter().map(|&a| ConfigurationValue::Number(a as f64)).collect() ))]);
		let second= ConfigurationValue::Object("DOR".to_string(), vec![("order".to_string(), ConfigurationValue::Array( order.iter().map(|&a| ConfigurationValue::Number(a as f64)).collect() ))]);

		let first=new_routing(RoutingBuilderArgument{cv: &first,..arg});
		let second=new_routing(RoutingBuilderArgument{cv: &second,..arg});
		// let first=first.expect("There were no first");
		// let second=second.expect("There were no second");
		//let first_reserved_virtual_channels=first_reserved_virtual_channels.expect("There were no first_reserved_virtual_channels");
		//let second_reserved_virtual_channels=second_reserved_virtual_channels.expect("There were no second_reserved_virtual_channels");

		AdaptiveValiantClos{
			first,
			second,
			// selection_exclude_indirect_routers,
			first_reserved_virtual_channels,
			second_reserved_virtual_channels,
		}
	}
}
