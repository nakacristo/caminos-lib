
use std::cell::RefCell;
use ::rand::{Rng,rngs::StdRng};
use quantifiable_derive::Quantifiable;//the derive macro
use crate::config_parser::ConfigurationValue;
use crate::topology::{Topology,Location};
use crate::routing::{RoutingInfo,Routing,CandidateEgress,RoutingBuilderArgument,RoutingNextCandidates};

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
	fn coordinated_routing_record(&self, coordinates_a:&[usize], coordinates_b:&[usize], _rng: Option<&RefCell<StdRng>>)->Vec<i32>
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
	fn coordinated_routing_record(&self, coordinates_a:&[usize], coordinates_b:&[usize], rng: Option<&RefCell<StdRng>>)->Vec<i32>
	{
		//In a Torus the routing record is for every difference of coordinates `d`, the minimum among `d` and `side-d` with the appropiate sign.
		(0..coordinates_a.len()).map(|i|{
			//coordinates_b[i] as i32-coordinates_a[i] as i32
			let side=self.cartesian_data.sides[i] as i32;
			let a=(side + coordinates_b[i] as i32-coordinates_a[i] as i32) % side;
			let b=(side + coordinates_a[i] as i32-coordinates_b[i] as i32) % side;
			if a==b
			{
				if let Some(rng)=rng
				{
					//let r=rng.borrow_mut().gen_range(0,2);//rand-0.4
					let r=rng.borrow_mut().gen_range(0..2);//rand-0.8
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
			//let dimension=port/2;
			//let delta=if port%2==0 { -1i32 as usize } else { 1 };
			let mut coordinates=self.cartesian_data.unpack(router_index);
			//coordinates[dimension]=coordinates[dimension].wrapping_add(delta);
			//if coordinates[dimension]>=self.cartesian_data.sides[dimension]
			//{
			//	return Location::None;
			//}
			let side=self.cartesian_data.sides[dimension];
			//coordinates[dimension]=(coordinates[dimension]+side+delta)%side;
			coordinates[dimension]=(coordinates[dimension]+offset+1)%side;
			let n_index=self.cartesian_data.pack(&coordinates);
			//let n_port= if delta==1
			//{
			//	dimension*2
			//}
			//else
			//{
			//	dimension*2+1
			//};
			let n_port= (side-2-offset) + (port-offset);
			return (Location::RouterPort{router_index:n_index, router_port:n_port},dimension);
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
	fn coordinated_routing_record(&self, coordinates_a:&[usize], coordinates_b:&[usize], _rng: Option<&RefCell<StdRng>>)->Vec<i32>
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
		let servers_per_router=servers_per_router.expect("There were no servers_per_router");
		//println!("servers_per_router={}",servers_per_router);
		Hamming{
			cartesian_data: CartesianData::new(&sides),
			servers_per_router,
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
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, _rng: &RefCell<StdRng>) -> RoutingNextCandidates
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
						return RoutingNextCandidates{candidates:r,idempotent:true};
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
			return RoutingNextCandidates{candidates:r,idempotent:true};
		}
	}
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, toology:&dyn Topology, current_router:usize, target_server:usize)
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_server:usize, rng: &RefCell<StdRng>)
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		//DOR needs cartesian data in the topology, which could be a dragonfly or whatever...
		let cartesian_data=topology.cartesian_data().expect("DOR requires a Cartesian topology");
		let up_current=cartesian_data.unpack(current_router);
		let up_target=cartesian_data.unpack(target_router);
		//let routing_record=(0..up_current.len()).map(|i|up_target[i] as i32-up_current[i] as i32).collect();//FIXME: torus
		let routing_record=topology.coordinated_routing_record(&up_current,&up_target,Some(rng));
		//println!("routing record from {} to {} is {:?}",current_router,target_router,routing_record);
		routing_info.borrow_mut().routing_record=Some(routing_record);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, _target_server:usize, _rng: &RefCell<StdRng>)
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
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &RefCell<StdRng>)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _num_virtual_channels:usize, _rng:&RefCell<StdRng>)
	{
	}
	fn statistics(&self, _cycle:usize) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:usize)
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
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, _rng: &RefCell<StdRng>) -> RoutingNextCandidates
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
						return RoutingNextCandidates{candidates:r,idempotent:true};
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
				return RoutingNextCandidates{candidates:r,idempotent:true}
			}
			else
			{
				let vcs=(0..num_virtual_channels).filter(|vc|!self.randomized_reserved_virtual_channels.contains(vc));
				//return vcs.flat_map(|vc| best.iter().map(|p|(*p,vc)).collect::<Vec<(usize,usize)>>()).collect();
				let r= vcs.flat_map(|vc| best.iter().map(|p|CandidateEgress::new(*p,vc)).collect::<Vec<_>>()).collect();
				return RoutingNextCandidates{candidates:r,idempotent:true}
			};
		}
	}
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, toology:&dyn Topology, current_router:usize, target_server:usize)
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_server:usize, rng: &RefCell<StdRng>)
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
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
		 	let t=rng.borrow_mut().gen_range(0..side);
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
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_server:usize, rng: &RefCell<StdRng>)
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
						let (target_location,_link_class)=topology.server_neighbour(target_server);
						let target_router=match target_location
						{
							Location::RouterPort{router_index,router_port:_} =>router_index,
							_ => panic!("The server is not attached to a router"),
						};
						Some(target_router)
					};
					while r==0 && offset<self.randomized.len()-1
					{
						offset+=1;
						let dim=self.randomized[offset];
						//XXX Should we skip if current[dim]==target[dim]?
						let side=cartesian_data.sides[dim];
						let t=rng.borrow_mut().gen_range(0..side);
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
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &RefCell<StdRng>)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _num_virtual_channels:usize, _rng:&RefCell<StdRng>)
	{
	}
	fn statistics(&self, _cycle:usize) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:usize)
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
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, _rng: &RefCell<StdRng>) -> RoutingNextCandidates
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
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						//return vec![CandidateEgress::new(i,s)];
						let r= available_virtual_channels.map(|vc| CandidateEgress::new(i,vc)).collect();
						return RoutingNextCandidates{candidates:r,idempotent:true};
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
			return RoutingNextCandidates{candidates:r,idempotent:true};
		}
	}
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, toology:&dyn Topology, current_router:usize, target_server:usize)
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_server:usize, rng: &RefCell<StdRng>)
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		//O1TURN needs cartesian data in the topology, which could be a dragonfly or whatever...
		let cartesian_data=topology.cartesian_data().expect("O1TURN requires a Cartesian topology");
		let up_current=cartesian_data.unpack(current_router);
		let up_target=cartesian_data.unpack(target_router);
		//let routing_record=(0..up_current.len()).map(|i|up_target[i] as i32-up_current[i] as i32).collect();//FIXME: torus
		let routing_record=topology.coordinated_routing_record(&up_current,&up_target,Some(rng));
		//println!("routing record from {} to {} is {:?}",current_router,target_router,routing_record);
		routing_info.borrow_mut().routing_record=Some(routing_record);
		routing_info.borrow_mut().selections=Some(vec![{
			rng.borrow_mut().gen_range(0..2)
		}]);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, current_port:usize, _target_server:usize, _rng: &RefCell<StdRng>)
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
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &RefCell<StdRng>)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _num_virtual_channels:usize, _rng:&RefCell<StdRng>)
	{
	}
	fn statistics(&self, _cycle:usize) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:usize)
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
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, _rng: &RefCell<StdRng>) -> RoutingNextCandidates
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		let distance=topology.distance(current_router,target_router);
		if distance==0
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
						return RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true}
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
		RoutingNextCandidates{candidates:r,idempotent:true}
	}
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, toology:&dyn Topology, current_router:usize, target_server:usize)
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _rng: &RefCell<StdRng>)
	{
		routing_info.borrow_mut().selections=Some(vec![self.allowed_deroutes as i32]);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_server:usize, _rng: &RefCell<StdRng>)
	{
		//let cartesian_data=topology.cartesian_data().expect("OmniDimensionalDeroute requires a Cartesian topology");
		if let (Location::RouterPort{router_index: previous_router,router_port:_},_link_class)=topology.neighbour(current_router,current_port)
		{
			//let up_current=cartesian_data.unpack(current_router);
			//let up_previous=cartesian_data.unpack(previous_router);
			let (target_location,_link_class)=topology.server_neighbour(target_server);
			let target_router=match target_location
			{
				Location::RouterPort{router_index,router_port:_} =>router_index,
				_ => panic!("The server is not attached to a router"),
			};
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
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &RefCell<StdRng>)
	{
	}
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _num_virtual_channels:usize, _rng:&RefCell<StdRng>)
	{
	}
	fn statistics(&self, _cycle:usize) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:usize)
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

