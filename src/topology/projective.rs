
use ::rand::{rngs::StdRng};
use std::fmt::Debug;
use quantifiable_derive::Quantifiable;//the derive macro
use crate::{
	error,source_location,
	quantify::Quantifiable,
	topology::{Topology,Location,CartesianData,TopologyBuilderArgument},
	config_parser::ConfigurationValue,
	error::{Error,SourceLocation},
};

//struct<F:FiniteField> ProjectivePlanePoint<F>
//{
//	point : [F,F,F],
//}
//
//struct ProjectivePlane
//{
//	//points: Vec<ProjectivePlanePoint<F>>
//}

pub trait Geometry
{
	type Point;
	type Line;
	//type IncidentPoints: Iterator<Item=Self::Point>;
	//type IncidentLines: Iterator<Item=Self::Line>;
	//fn incident_points(&self, line:&Self::Line) -> Self::IncidentPoints;
	//fn incident_lines(&self, point:&Self::Point) -> Self::IncidentLines;
	fn amount_points(&self) -> usize;
	fn amount_lines(&self) -> usize;
	fn point_by_index(&self, index:usize) -> Option<Self::Point>;
	fn line_by_index(&self, index:usize) -> Option<Self::Line>;
	fn index_of_point(&self, point:&Self::Point) -> usize;
	fn index_of_line(&self, point:&Self::Line) -> usize;
	fn is_incident(&self, line:&Self::Line, point:&Self::Point) -> bool;
}

pub trait SelfDualGeometry
{
	type Point;
	//type Incident : Iterator<Item=Self::Point>;
	//fn incident_points(&self, point:&Self::Point) -> Self::Incident;
	fn size(&self) -> usize;
	fn point_by_index(&self, index:usize) -> Option<Self::Point>;
	fn index_of_point(&self, point:&Self::Point) -> usize;
	fn is_incident(&self, line:&Self::Point, point:&Self::Point) -> bool;
}

impl<G:SelfDualGeometry> Geometry for G
{
	type Point=G::Point;
	type Line=G::Point;
	fn amount_points(&self) -> usize
	{
		self.size()
	}
	fn amount_lines(&self) -> usize
	{
		self.size()
	}
	fn point_by_index(&self, index:usize) -> Option<Self::Point>
	{
		self.point_by_index(index)
	}
	fn line_by_index(&self, index:usize) -> Option<Self::Line>
	{
		self.point_by_index(index)
	}
	fn index_of_point(&self, point:&Self::Point) -> usize
	{
		self.index_of_point(point)
	}
	fn index_of_line(&self, point:&Self::Line) -> usize
	{
		self.index_of_point(point)
	}
	fn is_incident(&self, line:&Self::Line, point:&Self::Point) -> bool
	{
		self.is_incident(line,point)
	}
}

pub trait FlatGeometry : Debug + Quantifiable
{
	fn amount_points(&self) -> usize;
	fn amount_lines(&self) -> usize;
	fn is_incident(&self, line:usize, point:usize) -> Result<bool,Error>;
}

///A projective plane for integer modulo prime p implemented with integers 0..p.
#[derive(Debug,Quantifiable)]
struct ProjectivePlaneZp
{
	prime: usize,
}

impl SelfDualGeometry for ProjectivePlaneZp
{
	type Point = [usize;3];
	//type Incident = ProjectivePlaneZpIncident;
	//fn incident(&self, point:&Self::Point) -> Self::Incident
	//{
	//	ProjectivePlaneZpIncident{
	//		prime: self.prime,
	//		origin: point.clone(),
	//		current: [0,0,0],
	//	}
	//}
	fn size(&self) -> usize
	{
		self.prime*self.prime+self.prime+1
	}
	fn point_by_index(&self, index:usize) -> Option<Self::Point>
	{
		let mut offset=index;
		if offset==0
		{
			return Some([1,0,0]);
		}
		offset-=1;
		if offset<self.prime
		{
			return Some([offset,1,0]);
		}
		offset-=self.prime;
		if offset<self.prime*self.prime
		{
			return Some([offset % self.prime, offset / self.prime, 1]);
		}
		None
	}
	fn index_of_point(&self, point:&Self::Point) -> usize
	{
		//assuming point is valid
		if point[1]==0 && point[2]==0
		{
			return 0;
		}
		if point[2]==0
		{
			return 1 + point[0];
		}
		return 1+self.prime+point[0]+point[1]*self.prime;
	}
	fn is_incident(&self, line:&Self::Point, point:&Self::Point) -> bool
	{
		let prod = (0..3).map(|k|line[k]*point[k]).sum::<usize>() % self.prime;
		prod == 0
	}
}

//struct ProjectivePlaneZpIncident
//{
//	prime: usize,
//	origin: [usize;3],
//	current: [usize;3],
//}
//
//impl Iterator for ProjectivePlaneZpIncident
//{
//	type Item = [usize;3];
//	fn next(&mut self) -> Option<Self::Item>
//	{
//		loop{
//			self.current[0]+=1;
//			if self.current[0]>=self.prime
//			{
//				self.current[1]+=1;
//				if self.current[1]>=self.prime
//				{
//					return None;
//				}
//				self.current[0]=0;
//				if self.current[1]==2 && self.current[2]==0
//				{
//					self.current[1]=0;
//					self.current[2]=1;
//				}
//			}
//			//if self.current[0]==2 && self.current[1]==0 && self.current[2]==0
//			if self.current==[2,0,0]
//			{
//				self.current[0]=0;
//				self.current[1]=1;
//			}
//			//Some(self.current)
//			let prod = (0..3).map(|k|self.origin[k]*self.current[k]).sum::<usize>() % self.prime;
//			if prod==0
//			{
//				return Some(self.current);
//			}
//		}
//	}
//}

trait ProjectivePlane:Debug + Quantifiable
{
	fn size(&self) -> usize;
}

impl ProjectivePlane for ProjectivePlaneZp
{
	fn size(&self) -> usize
	{
		SelfDualGeometry::size(self)
	}
}

impl<G:Geometry + Debug + Quantifiable> FlatGeometry for G
{
	fn amount_points(&self) -> usize
	{
		Geometry::amount_points(self)
	}
	fn amount_lines(&self) -> usize
	{
		Geometry::amount_lines(self)
	}
	fn is_incident(&self, line:usize, point:usize) -> Result<bool,Error>
	{
		Ok(Geometry::is_incident(self,&self.line_by_index(line).ok_or_else(||error!(undetermined))?,&self.point_by_index(point).ok_or_else(||error!(undetermined))?))
	}
}


#[derive(Debug,Quantifiable)]
pub struct FlatGeometryCache
{
	pub geometry: Box<dyn FlatGeometry>,
	///`lines_by_point[point][point_index]=(line,line_index)` satisfying `points_by_line[line][line_index]=(point,point_index)`.
	pub lines_by_point: Vec<Vec<(usize,usize)>>,
	///`points_by_line[line][line_index]=(point,point_index)` satisfying `lines_by_point[point][point_index]=(line,line_index)`.
	pub points_by_line: Vec<Vec<(usize,usize)>>,
}

impl FlatGeometryCache
{
	pub fn new_prime(prime:usize) -> Result<FlatGeometryCache,Error>
	{
		for divisor in 2..prime
		{
			if prime % divisor ==0
			{
				return Err(error!(undetermined));
			}
			if divisor*divisor>=prime
			{
				break;
			}
		}
		let plane=ProjectivePlaneZp { prime };
		let n = ProjectivePlane::size(&plane);
		let mut lines_by_point:Vec<Vec<(usize,usize)>>=(0..n).map(|point|{
			(0..n).filter_map(|line|{
				//if plane.is_incident(line,point) { Some((line,0)) } else { None }
				if FlatGeometry::is_incident(&plane,line,point).expect("the points should be in range") { Some((line,0)) } else { None }
			}).collect()
		}).collect();
		for point in 0..n
		{
			let deg=lines_by_point[point].len();
			for point_index in 0..deg
			{
				let (line,_)=lines_by_point[point][point_index];
				//find the point in the line
				let (line_index,_) = lines_by_point[line].iter().enumerate().find(|(_line_index,(some_point,_))|*some_point==point).expect("could not find the endpoint");
				lines_by_point[point][point_index]=(line,line_index);
			}
		}
		let points_by_line=lines_by_point.clone();//because self-dual
		Ok(FlatGeometryCache{
			geometry: Box::new(plane),
			lines_by_point,
			points_by_line,
		})
	}
	fn incident_points(&self, line:usize) -> Result<&Vec<(usize,usize)>,Error>
	{
		if line>=self.points_by_line.len()
		{
			Err(error!(undetermined))
		} else {
			Ok(&self.points_by_line[line])
		}
	}
	fn incident_lines(&self, point:usize) -> Result<&Vec<(usize,usize)>,Error>
	{
		if point>=self.lines_by_point.len()
		{
			Err(error!(undetermined))
		} else {
			Ok(&self.lines_by_point[point])
		}
	}
}


///The projective topology.
///Erdos, Renyi graph
///Or Brown graph
///Used by Brahme as a network topology, with other definition.
#[derive(Debug,Quantifiable)]
pub struct Projective
{
	//plane: Box<dyn ProjectivePlane>,
	plane: FlatGeometryCache,
	servers_per_router: usize,
}

impl Topology for Projective
{
	fn num_routers(&self) -> usize
	{
		//self.plane.size()
		self.plane.geometry.amount_points()
	}
	fn num_servers(&self) -> usize
	{
		self.servers_per_router * self.num_routers()
	}
	///Neighbours of a router: Location+link class index
	///Routers should be before servers
	fn neighbour(&self, router_index:usize, port:usize) -> (Location,usize)
	{
		let neighs = self.plane.incident_points(router_index).unwrap_or_else(|_|panic!("invalid router_index={}",router_index));
		if port<neighs.len()
		{
			let (neighbour_router, neighbour_port) = neighs[port];
			if neighbour_router == router_index
			{
				//Remove loops.
				(Location::None,0)
			}
			else
			{
				(Location::RouterPort{
					router_index: neighbour_router,
					router_port: neighbour_port,
				},0)
			}
		}
		else
		{
			let offset = port - neighs.len();
			(Location::ServerPort(offset+router_index*self.servers_per_router),1)
		}
	}
	///The neighbour of a server: Location+link class index
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let router_index = server_index/self.servers_per_router;
		let router_port = (server_index % self.servers_per_router) + self.degree(router_index);
		(Location::RouterPort{
			router_index,
			router_port,
		},1)
	}
	///the greatest distance from server to server
	fn diameter(&self) -> usize
	{
		2
	}
	//from servers to different servers
	//fn average_distance(&self) -> f32
	//{
	//	2f32
	//}
	///Distance from a router to another.
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		if origin==destination
		{
			0
		} else if self.plane.geometry.is_incident(origin,destination).expect("origin and destination should be in range")
		{
			1
		} else
		{
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
		//assumed regular
		self.plane.incident_points(0).expect("must have some point").len()
	}
	fn minimum_degree(&self) -> usize
	{
		//assumed regular
		self.plane.incident_points(0).expect("must have some point").len()
	}
	/// Number of ports used to other routers.
	fn degree(&self, router_index: usize) -> usize
	{
		self.plane.incident_points(router_index).expect("must have some point").len()
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

impl Projective
{
	pub fn new(arg:TopologyBuilderArgument) -> Projective
	{
		let mut prime=None;
		let mut servers_per_router=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="Projective"
			{
				panic!("A Projective must be created from a `Projective` object not `{}`",cv_name);
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
		Projective{
			plane: FlatGeometryCache::new_prime(prime).unwrap_or_else(|_|panic!("{} is not prime, which is required for the Projective topology",prime)),
			servers_per_router,
		}
	}
}


///Taking the Levi graph of the projective plane as topology. Both points and lines are routers with attached servers.
///We put the points in the first offsets, the lines next.
#[derive(Debug,Quantifiable)]
pub struct LeviProjective
{
	plane: FlatGeometryCache,
	servers_per_router: usize,
}

impl Topology for LeviProjective
{
	fn num_routers(&self) -> usize
	{
		//self.plane.size()
		self.plane.geometry.amount_points() + self.plane.geometry.amount_lines()
	}
	fn num_servers(&self) -> usize
	{
		self.servers_per_router * self.num_routers()
	}
	///Neighbours of a router: Location+link class index
	///Routers should be before servers
	fn neighbour(&self, router_index:usize, port:usize) -> (Location,usize)
	{
		let np = self.plane.geometry.amount_points();
		if router_index < np
		{
			//The router is a point
			let neighs = self.plane.incident_lines(router_index).unwrap_or_else(|_|panic!("invalid router_index={}",router_index));
			if port<neighs.len()
			{
				let (neighbour_line, neighbour_port) = neighs[port];
				(Location::RouterPort{
					router_index: neighbour_line + np,
					router_port: neighbour_port,
				},0)
			}
			else
			{
				let offset = port - neighs.len();
				(Location::ServerPort(offset+router_index*self.servers_per_router),1)
			}
		} else {
			//The router is a line
			let line = router_index - np;
			let neighs = self.plane.incident_points(line).unwrap_or_else(|_|panic!("invalid router_index={}",router_index));
			if port<neighs.len()
			{
				let (neighbour_point, neighbour_port) = neighs[port];
				(Location::RouterPort{
					router_index: neighbour_point,
					router_port: neighbour_port,
				},0)
			}
			else
			{
				let offset = port - neighs.len();
				(Location::ServerPort(offset+router_index*self.servers_per_router),1)
			}
		}
	}
	///The neighbour of a server: Location+link class index
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let router_index = server_index/self.servers_per_router;
		let router_port = (server_index % self.servers_per_router) + self.degree(router_index);
		(Location::RouterPort{
			router_index,
			router_port,
		},1)
	}
	///the greatest distance from server to server
	fn diameter(&self) -> usize
	{
		3
	}
	// ///from servers to different servers
	// fn average_distance(&self) -> f32
	// {
	// 	//2f32
	// 	todo!()
	// }
	///Distance from a router to another.
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		if origin==destination
		{
			0
		} else {
			let np = self.plane.geometry.amount_points();
			let point_count = if origin<np {1} else {0} + if destination<np {1} else {0};
			if point_count==1
			{
				let point = origin.min(destination);
				let line = origin.max(destination) - np;
				if self.plane.geometry.is_incident(line,point).expect("origin and destination should be in range")
				{
					1
				} else
				{
					3
				}
			}
			else
			{
				2
			}
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
		//assumed regular
		let dp = self.plane.incident_lines(0).expect("must have some point").len();
		let dl = self.plane.incident_points(0).expect("must have some line").len();
		usize::max(dp,dl)
	}
	fn minimum_degree(&self) -> usize
	{
		//assumed regular
		let dp = self.plane.incident_lines(0).expect("must have some point").len();
		let dl = self.plane.incident_points(0).expect("must have some line").len();
		usize::min(dp,dl)
	}
	/// Number of ports used to other routers.
	fn degree(&self, router_index: usize) -> usize
	{
		let np = self.plane.geometry.amount_points();
		if router_index < np
		{
			self.plane.incident_lines(router_index).expect("must have some point").len()
		}
		else
		{
			self.plane.incident_points(router_index - np).expect("must have some line").len()
		}
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

impl LeviProjective
{
	pub fn new(arg:TopologyBuilderArgument) -> LeviProjective
	{
		let mut prime=None;
		let mut servers_per_router=None;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=arg.cv
		{
			if cv_name!="LeviProjective"
			{
				panic!("A LeviProjective must be created from a `LeviProjective` object not `{}`",cv_name);
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
		LeviProjective{
			plane: FlatGeometryCache::new_prime(prime).unwrap_or_else(|_|panic!("{} is not prime, which is required for the Projective topology",prime)),
			servers_per_router,
		}
	}
}

