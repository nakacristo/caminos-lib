
use ::rand::{rngs::StdRng};
use std::fmt::Debug;
use std::convert::TryInto;
use quantifiable_derive::Quantifiable;//the derive macro
use super::{
	Topology,TopologyBuilderArgument,CartesianData,Location,
};
use crate::{
	config_parser::ConfigurationValue,
	quantify::Quantifiable,
};

//For the slimfly we need for x,y in the field, i integer
//pow(x,i)
//x-y
//x*y
//field.size()
pub trait FlatRing : Debug + Quantifiable
{
	fn size(&self) -> usize;
	// Given an `integer` find the element in the ring corresponding to `integer` times the multiplicative unit of the ring.
	fn index_from_i32(&self, integer: i32) -> usize;
	fn add(&self, a:usize, b:usize) -> usize;
	fn sub(&self, a:usize, b:usize) -> usize;
	fn mul(&self, a:usize, b:usize) -> usize;
	fn pow(&self, a:usize, exp:u32) -> usize
	{
		let mut current=1;
		let mut rem_exp=exp;
		let mut factor=a;
		while rem_exp > 0
		{
			if rem_exp % 2 == 1
			{
				current=self.mul(current,factor);
			}
			rem_exp/=2;
			factor = self.mul(factor,factor);
		}
		current
	}
	///return (x,y) with a^x=y and y with the least possible representation.
	///Assuming 0=zero, 1=one.
	fn order(&self, a:usize) -> (usize,usize)
	{
		let mut ret=(1,a);
		if a==0
		{
			return ret;
		}
		let mut current = a;
		//let n = self.size();
		let mut exp=1;
		loop{
			current=self.mul(current,a);
			if current==ret.1
			{
				//We have found a cycle
				return ret;
			}
			exp+=1;
			if current==1
			{
				return (exp,current);
			}
			if current<ret.1
			{
				//update
				ret=(exp,current);
			}
		}
	}
	fn is_primitive(&self, a:usize) -> bool
	{
		let n = self.size();
		//FIXME: n even
		let half:u32 = ((n-1)/2).try_into().unwrap();
		let prev = self.pow(a,half);
		prev!=1 && self.mul(prev,prev)==1
	}
}

#[derive(Debug,Quantifiable)]
struct IntegerIdealRing
{
	modulo: usize
}

impl FlatRing for IntegerIdealRing
{
	fn size(&self) -> usize
	{
		self.modulo
	}
	fn index_from_i32(&self, integer: i32) -> usize
	{
		let mut ret:i32 = integer;
		let m :i32 = self.modulo as i32;
		while ret<0
		{
			ret += m;
		}
		ret %= m;
		ret as usize
	}
	fn add(&self, a:usize, b:usize) -> usize
	{
		(a+b) % self.modulo
	}
	fn sub(&self, a:usize, b:usize) -> usize
	{
		(self.modulo + a -b) % self.modulo
	}
	fn mul(&self, a:usize, b:usize) -> usize
	{
		(a*b) % self.modulo
	}
}


struct SlimFlyCoordinates
{
	//in field
	local: usize,
	//in field
	global: usize,
	//0 o 1
	block: usize,
}

impl SlimFlyCoordinates
{
	fn unpack(index:usize,size:usize) -> SlimFlyCoordinates
	{
		let local = index % size;
		let other = index / size;
		let global = other % size;
		let block = other / size;
		SlimFlyCoordinates{
			local,
			global,
			block,
		}
	}
	fn pack(&self,size:usize) -> usize
	{
		(self.block * size + self.global)*size + self.local
	}
}


#[derive(Debug,Quantifiable)]
pub struct SlimFly
{
	field: Box<dyn FlatRing>,
	primitive: usize,
	servers_per_router: usize,
	paley_sets: [Vec<usize>;2],
	neg_paley_sets: [Vec<usize>;2],
}

impl Topology for SlimFly
{
	fn num_routers(&self) -> usize
	{
		//self.plane.size()
		let n=self.field.size();
		n*n*2
	}
	fn num_servers(&self) -> usize
	{
		self.servers_per_router * self.num_routers()
	}
	///Neighbours of a router: Location+link class index
	///Routers should be before servers
	fn neighbour(&self, router_index:usize, port:usize) -> (Location,usize)
	{
		let n = self.field.size();
		let router_coords = SlimFlyCoordinates::unpack(router_index,n);
		if port < self.paley_sets[0].len()
		{
			//local link. class 0.
			//let neighbour_local_partial = self.field.add(router_coords.local,self.paley_set[port]);
			//let neighbour_local = if router_coords.block==0 { neighbour_local_partial } else { self.field.mul(neighbour_local_partial,self.primitive) };
			let neighbour_local= self.field.add(router_coords.local,self.paley_sets[router_coords.block][port]);
			return (Location::RouterPort{
				router_index: SlimFlyCoordinates{local:neighbour_local,..router_coords}.pack(n),
				//router_port: self.paley_set.len()-1-port,
				//router_port: self.neg_paley_set[port],
				router_port: self.neg_paley_sets[router_coords.block][port],
			},0);
		}
		let offset=port - self.paley_sets[0].len();
		if offset < self.field.size()
		{
			//global link. class 1.
			//y2=y1 - x1*x2
			//y1=y2 + x1*x2
			let global_product = self.field.mul(router_coords.global,offset);
			let neighbour_local = if router_coords.block==0
			{
				self.field.sub(router_coords.local,global_product)
			}
			else
			{
				self.field.add(router_coords.local,global_product)
			};
			return (Location::RouterPort{
				router_index: SlimFlyCoordinates{local:neighbour_local,global:offset,block:1-router_coords.block}.pack(n),
				router_port: self.paley_sets[0].len() + router_coords.global,
			},1);
		}
		let offset = offset - self.field.size();
		(Location::ServerPort(offset+router_index*self.servers_per_router),2)
		//let neighs = self.plane.incident_points(router_index).unwrap_or_else(|_|panic!("invalid router_index={}",router_index));
		//if port<neighs.len()
		//{
		//	let (neighbour_router, neighbour_port) = neighs[port];
		//	if neighbour_router == router_index
		//	{
		//		//Remove loops.
		//		(Location::None,0)
		//	}
		//	else
		//	{
		//		(Location::RouterPort{
		//			router_index: neighbour_router,
		//			router_port: neighbour_port,
		//		},0)
		//	}
		//}
		//else
		//{
		//	let offset = port - neighs.len();
		//	(Location::ServerPort(offset+router_index*self.servers_per_router),1)
		//}
	}
	///The neighbour of a server: Location+link class index
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let router_index = server_index/self.servers_per_router;
		let router_port = (server_index % self.servers_per_router) + self.degree(router_index);
		(Location::RouterPort{
			router_index,
			router_port,
		},2)
	}
	///the greatest distance from server to server
	fn diameter(&self) -> usize
	{
		2
	}
	///Distance from a router to another.
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		if origin==destination
		{
			0
		} else
		{
			let origin_coords = SlimFlyCoordinates::unpack(origin,self.field.size());
			let destination_coords = SlimFlyCoordinates::unpack(destination,self.field.size());
			if origin_coords.block==destination_coords.block && origin_coords.global==destination_coords.global
			{
				let local_diff=self.field.sub(origin_coords.local,destination_coords.local);
				//let shifted_diff = if origin_coords.block==0 { local_diff } else { self.field.mul(local_diff,self.primitive) };
				//if self.paley_set.contains(&shifted_diff)
				if self.paley_sets[origin_coords.block].contains(&local_diff)
				{
					//local link (class 0)
					return 1;
				}
			}
			if origin_coords.block!=destination_coords.block
			{
				let (left,right) = if origin_coords.block==0 {(origin_coords,destination_coords)} else {(destination_coords,origin_coords)};
				let local_diff = self.field.sub(left.local,right.local);
				let global_prod = self.field.mul(left.global,right.global);
				if local_diff==global_prod
				{
					//global link (class 1)
					return 1;
				}
			}
			2
		}
	}
	///Number of shortest paths from a router to another.
	fn amount_shortest_paths(&self,_origin:usize,_destination:usize) -> usize
	{
		todo!();
	}
	///Average number of shortest paths from a router to another.
	fn average_amount_shortest_paths(&self) -> f32
	{
		todo!();
	}
	//fn eigenvalue_powerdouble(&self) -> f32
	fn maximum_degree(&self) -> usize
	{
		self.paley_sets[0].len() + self.field.size()
	}
	fn minimum_degree(&self) -> usize
	{
		self.paley_sets[0].len() + self.field.size()
	}
	/// Number of ports used to other routers.
	fn degree(&self, _router_index: usize) -> usize
	{
		self.paley_sets[0].len() + self.field.size()
	}
	fn ports(&self, router_index: usize) -> usize
	{
		self.degree(router_index) + self.servers_per_router
	}
	///Specific for some toologies, but must be checkable for anyone
	fn cartesian_data(&self) -> Option<&CartesianData>
	{
		None
	}
	///Specific for some toologies, but must be checkable for anyone
	fn coordinated_routing_record(&self, _coordinates_a:&[usize], _coordinates_b:&[usize], _rng: Option<&mut StdRng>)->Vec<i32>
	{
		unimplemented!()
	}
	///Specific for some toologies, but must be checkable for anyone
	/// Indicates if going from input_port to output_port implies a direction change. Used for the bubble routing.
	fn is_direction_change(&self, _router_index:usize, _input_port: usize, _output_port: usize) -> bool
	{
		false
	}
	fn up_down_distance(&self,_origin:usize,_destination:usize) -> Option<(usize,usize)>
	{
		None
	}
}

impl SlimFly
{
	pub fn new(arg:TopologyBuilderArgument) -> SlimFly
	{
		let mut prime=None;
		let mut primitive=None;
		let mut servers_per_router=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="SlimFly"
			{
				panic!("A SlimFly must be created from a `SlimFly` object not `{}`",cv_name);
			}
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"prime" => match value
					{
						&ConfigurationValue::Number(f) => prime=Some(f as usize),
						_ => panic!("bad value for prime"),
					},
					"primitive" => match value
					{
						&ConfigurationValue::Number(f) => primitive=Some(f as usize),
						_ => panic!("bad value for primitive"),
					},
					"servers_per_router" => match value
					{
						&ConfigurationValue::Number(f) => servers_per_router=Some(f as usize),
						_ => panic!("bad value for servers_per_router"),
					},
					"legend_name" => (),
					_ => panic!("Nothing to do with field {} in RandomRegularGraph",name),
				}
			}
		}
		else
		{
			panic!("Trying to create a NeighboursLists from a non-Object");
		}
		let prime=prime.expect("There were no prime");
		let servers_per_router=servers_per_router.expect("There were no servers_per_router");
		let field = IntegerIdealRing{modulo:prime};
		let primitive=primitive.unwrap_or_else(||{
			let n=field.size();
			(2..n).find(|x|field.is_primitive(*x)).unwrap_or_else(||panic!("Could not find a primtive element in the ring {:?}",field))
		});
		let epsilon:i32 = {
			let p4 = prime % 4;
			if p4==3 {-1} else {1}
		};
		let paley_set:Vec<usize>=match epsilon
		{
			1 =>
			{
				let limit :u32 = (prime as u32-1)/2;
				(0..limit).map(|k|field.pow(primitive,2*k)).collect()
			},
			-1 =>
			{
				let limit :u32= (prime as u32-3)/4;
				(0..=limit).map(|k|2*k).chain( (0..=limit).map(|k|(prime as u32-1)/2 + 2*k) ).map(|exp|field.pow(primitive,exp)).collect()
			},
			0 =>
			{
				let limit :u32= prime as u32/2;
				(0..limit).map(|k|field.pow(primitive,2*k)).collect()
			},
			_ => panic!("{} cannot be a prime",prime),
		};
		println!("primitive={} paley_set={:?} len={} (q-eps)/2={}",primitive,paley_set,paley_set.len(),(prime as i32-epsilon)/2);
		let second_paley_set=paley_set.iter().map(|x|field.mul(*x,primitive)).collect();
		let paley_sets:[Vec<usize>;2]=[paley_set,second_paley_set];
		//let neg_paley_sets=(0..=1).map(|b|(0..paley_sets[b].len()).map(|k|{
		//	let elem=paley_sets[b][k];
		//	let neg_elem=field.sub(0,elem);
		//	paley_sets[b].iter().enumerate().find(|(_,x)|**x==neg_elem).expect("the Paley should be circulant").0
		//}).collect()).collect::<Vec<_>>().try_into().unwrap();
		let neg_paley_sets=
		{
			let builder=|b:usize|{
				(0..paley_sets[b].len()).map(|k|{
					let elem=paley_sets[b][k];
					let neg_elem=field.sub(0,elem);
					paley_sets[b].iter().enumerate().find(|(_,x)|**x==neg_elem).expect("the Paley should be circulant").0
			}).collect()};
			[builder(0),builder(1)]
		};
		SlimFly{
			field: Box::new(field),
			primitive,
			servers_per_router,
			paley_sets,
			neg_paley_sets,
		}
	}
}



