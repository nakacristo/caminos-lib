
/*!

A Topology defines the way routers and servers are connected.

see [`new_topology`](fn.new_topology.html) for documentation on the configuration syntax of predefined topologies.

*/

pub mod operations;
pub mod cartesian;
pub mod neighbourslists;
pub mod dragonfly;
pub mod projective;
pub mod slimfly;
pub mod multistage;
pub mod megafly;

use std::fs::File;
use ::rand::{rngs::StdRng};
use std::io::{Write};

use quantifiable_derive::Quantifiable;//the derive macro
use self::cartesian::{Mesh,Torus,CartesianData,Hamming,AsCartesianTopology};
use self::neighbourslists::NeighboursLists;
use self::dragonfly::Dragonfly;
use self::projective::{Projective,LeviProjective};
use self::slimfly::SlimFly;
use self::multistage::MultiStage;
use crate::config_parser::ConfigurationValue;
use crate::matrix::Matrix;
use crate::quantify::Quantifiable;
use crate::Plugs;

/// Some things most uses of the topology module will use.
pub mod prelude
{
	pub use super::{Topology,Location,cartesian::CartesianData,TopologyBuilderArgument,new_topology,NeighbourRouterIteratorItem};
	pub use std::cell::{RefCell};
	pub use ::rand::rngs::StdRng;
}

///A location where a phit can be inserted.
///None is used for disconnected ports, for example in the `Mesh` topology.
#[derive(Clone,Debug,Quantifiable,Hash,Eq,PartialEq)]
pub enum Location
{
	RouterPort{
		router_index: usize,
		router_port: usize,
	},
	ServerPort(usize),
	None,
}

///Item for iterators of neighbour routers.
#[derive(Debug)]
pub struct NeighbourRouterIteratorItem
{
	///Port of the current router that goes to the neighbour.
	pub port_index:usize,
	///Link class of the link to the neighbour router.
	pub link_class:usize,
	///The index of the neighbour router.
	pub neighbour_router: usize,
	///The port index of the neighbour router corresponding to the same physical link.
	pub neighbour_port: usize,
}

///A topology describes how routers and servers are connected.
///The router `index` has `ports(index)` neighbours. The first `degree(index)` must be other routers.
pub trait Topology : Quantifiable + std::fmt::Debug
{
	fn num_routers(&self) -> usize;
	fn num_servers(&self) -> usize;
	// num_arcs is never used: deleted
	// fn num_arcs(&self) -> usize;
	///Neighbours of a router: Location+link class index
	///Routers should be before servers
	fn neighbour(&self, router_index:usize, port:usize) -> (Location,usize);
	///The neighbour of a server: Location+link class index
	//FIXME: What to do with BCube and similar?
	fn server_neighbour(&self, server_index:usize) -> (Location,usize);
	//diameter is only used in private projects...
	///the greatest distance from server to server
	fn diameter(&self) -> usize;
	//average distance is never used: deleted
	// ///from servers to different servers
	// fn average_distance(&self) -> f32;
	///Distance from a router to another.
	fn distance(&self,origin:usize,destination:usize) -> usize;
	///Number of shortest paths from a router to another.
	fn amount_shortest_paths(&self,origin:usize,destination:usize) -> usize;
	///Average number of shortest paths from a router to another.
	fn average_amount_shortest_paths(&self) -> f32;
	//fn arc_uniformity(&self) -> f32;
	//fn throughput(&self) -> f32;
	//fn get_arc_betweenness_matrix(&self) -> ??
	//fn distance_distribution(&self,origin:usize) -> Vec<usize>;
	//fn eigenvalue_powerdouble(&self) -> f32
	/**
	The maximum value returned by [degree]. You possibly want to override the default method to avoid its O(n) cost.
	**/
	fn maximum_degree(&self) -> usize
	{
		(0..self.num_routers()).map(|router_index|self.degree(router_index)).max().expect("calling maximum_degree without routers")
	}
	/**
	The minimum value returned by [degree]. You possibly want to override the default method to avoid its O(n) cost.
	**/
	fn minimum_degree(&self) -> usize
	{
		(0..self.num_routers()).map(|router_index|self.degree(router_index)).min().expect("calling minimum_degree without routers")
	}
	/// Number of ports used to other routers.
	/// This does not include non-connected ports.
	/// This should not be used as a range of valid ports. A non-connected port can be before some other valid port to a router.
	/// Use `neighbour_router_iter()' or `0..ports()' to iterate over valid ranges.
	fn degree(&self, router_index: usize) -> usize;
	fn ports(&self, router_index: usize) -> usize;
	//std::vector<std::vector<length> >* nonEdgeDistances()const;
	//length girth()const;
	///Iterate over the neighbour routers, skipping non-connected ports and ports towards servers.
	///You may want to reimplement this when implementing the trait for your type.
	fn neighbour_router_iter<'a>(&'a self, router_index:usize) -> Box<dyn Iterator<Item=NeighbourRouterIteratorItem> + 'a>
	{
		let np = self.ports(router_index);
		let iterator = (0..np).filter_map(move |port_index|{
			let (location,link_class) = self.neighbour(router_index,port_index);
			match location
			{
				Location::RouterPort {router_index: neighbour_router, router_port: neighbour_port} =>
				{
					Some(NeighbourRouterIteratorItem{port_index,link_class,neighbour_router,neighbour_port})
				},
				_ => None,
			}
		});
		Box::new(iterator)
	}
	
	///Specific for some topologies, but must be checkable for anyone
	fn cartesian_data(&self) -> Option<&CartesianData>;
	///Specific for some topologies, but must be checkable for anyone
	fn coordinated_routing_record(&self, _coordinates_a:&[usize], _coordinates_b:&[usize], _rng:Option<&mut StdRng>)->Vec<i32>
	{
		unimplemented!()
	}
	///Specific for some topologies, but must be checkable for anyone
	/// Indicates if going from input_port to output_port implies a direction change. Used for the bubble routing.
	fn is_direction_change(&self, _router_index:usize, _input_port: usize, _output_port: usize) -> bool { false }
	///For topologies containing the so called up/down paths. Other topologies should return always `None`.
	///If the return is `Some((u,d))` it means there is an initial up sub-path of length `u` followed by a down sub-path of length `d` starting at `origin` and ending at `destination`. A return value of `None` means there is no up/down path from `origin` to `destination`.
	///Some general guidelines, although it is not clear if they must hold always:
	/// * If there is a down path of length `d` then return `Some((0,d))`
	/// * If there is a up path of length `u` then return `Some((u,0))`
	/// * If `up_down_distance(s,t)=(u,d)` with `u>0` then some neighour `m` of `s` should have `up_down_distance(m,t)=(u-1,d)`
	/// * Return always a path of least `u+d`.
	/// * Minimize `u` before `d`?
	/// * If `up_down_distance(s,t)=(u,d)` then `up_down_distance(t,s)=(d,u)`?
	/// * In multistage networks `u-d` is the difference on levels and allows for some algebra.
	///Note that in general `u+d` is not an actual distance, since the triangular inequality does not hold.
	fn up_down_distance(&self,origin:usize,destination:usize) -> Option<(usize,usize)>;
	/// Information for Dragonfly-like networks.
	fn dragonfly_size(&self) -> Option<dragonfly::ArrangementSize> { None }

	///Breadth First Search to compute distances from a router to all others.
	///It may use weights, but it there are multiple paths with different distances it may give a non-minimal distance, since it is not Dijkstra.
	fn bfs(&self, origin:usize, class_weight:Option<&[usize]>) -> Vec<usize>
	{
		//Adapted from my code for other software.
		let n=self.num_routers();
		#[allow(non_snake_case)]
		let mut R=vec![<usize>::MAX;n];
		R[origin]=0;
		//let mut queue=vec![0;n];
		let queue_len=match class_weight
		{
			Some(v)=> n*v.len(),
			None => n,
		};
		let mut queue=vec![0;queue_len];
		let mut queue_read_index=0;//Next to read
		let mut queue_write_index=1;//Next to write
		queue[0]=origin;
		//while queue_read_index<n
		while queue_read_index<queue_write_index
		{
			let best=queue[queue_read_index];
			queue_read_index+=1;
			//let alt=R[best]+1;
			//let alt=R[best].saturating_add(1);
			//std::vector<vertex_index> nbor=neighbours(best);
			//let degree=self.degree(best);
			//for i in 0..degree
			//{
			//	match self.neighbour(best,i)
			//	{
			//		(Location::RouterPort{
			//			router_index,
			//			router_port: _,
			//		},link_class) =>
			//		{
			//			let weight= if let Some(ref v)=class_weight
			//			{
			//				if link_class>=v.len()
			//				{
			//					continue//next neighbour
			//				}
			//				let x=v[link_class];
			//				if x==<usize>::max_value()
			//				{
			//					continue//next neighbour
			//				}
			//				x
			//			}
			//			else
			//			{
			//				1
			//			};
			//			let alt=R[best].saturating_add(weight);
			//			if alt<R[router_index]
			//			{
			//				//println!("router_index={} n={} queue_write_index={} queue_read_index={}",router_index,n,queue_write_index,queue_read_index);
			//				R[router_index]=alt;
			//				queue[queue_write_index]=router_index;
			//				queue_write_index+=1;
			//			}
			//		}
			//		_ => panic!("what?"),
			//	}
			//}
			for NeighbourRouterIteratorItem{link_class,neighbour_router:router_index,..} in self.neighbour_router_iter(best)
			{
				let weight= if let Some(v)=class_weight
				{
					if link_class>=v.len()
					{
						continue//next neighbour
					}
					let x=v[link_class];
					if x==<usize>::MAX
					{
						continue//next neighbour
					}
					x
				}
				else
				{
					1
				};
				let alt=R[best].saturating_add(weight);
				if alt<R[router_index]
				{
					//println!("router_index={} n={} queue_write_index={} queue_read_index={}",router_index,n,queue_write_index,queue_read_index);
					R[router_index]=alt;
					queue[queue_write_index]=router_index;
					queue_write_index+=1;
				}
			}
		}
		return R;
	}
	
	/**
	Computes the diameter by checking all switch pairs.
	**/
	fn compute_diameter(&self) -> usize
	{
		let mut maximum=0;
		let n=self.num_routers();
		for source in 0..n
		{
			for target in 0..n
			{
				let d=self.distance(source,target);
				if d>maximum
				{
					maximum=d;
				}
			}
		}
		maximum
	}
	
	//Matrix<length>* Graph::computeDistanceMatrix()
	fn compute_distance_matrix(&self, class_weight:Option<&[usize]>) -> Matrix<usize>
	{
		//return floyd();
		let n=self.num_routers();
		let mut matrix=Matrix::constant(0,n,n);
		for i in 0..n
		{
			let d=self.bfs(i,class_weight);
			for j in 0..n
			{
				*matrix.get_mut(i,j)=d[j];
			}
		}
		return matrix;
	}

	fn floyd(&self) -> Matrix<usize>
	{
		// Implements Floyd–Warshall algorithm. This was adapted from a previous code for another software.
		//printf(">>Graph::computeDistanceMatrix\n");
		let n=self.num_routers();
		//Matrix<length>* matrix=new Matrix<length>(n,n);
		let mut matrix=Matrix::constant(<usize>::MAX/3,n,n);
		//vertex_index i,j,k;
		//length x;
		//for(i=0;i<n;i++)matrix->get(i,i)=0;
		for i in 0..n
		{
			*matrix.get_mut(i,i)=0;
		}
		//for(i=0;i<n;i++)
		for i in 0..n
		{
			// //std::vector<vertex_index> nbor=neighbours(i);
			// let degree=self.degree(i);
			// //for(j=0;j<nbor.size();j++)
			// for j in 0..degree
			// {
			// 	//matrix->get(i,nbor[j])=1;
			// 	match self.neighbour(i,j).0
			// 	{
			// 		Location::RouterPort{
			// 			router_index,
			// 			router_port: _,
			// 		} => *matrix.get_mut(i,router_index)=1,
			// 		_ => panic!("what?"),
			// 	}
			// }
			for NeighbourRouterIteratorItem{neighbour_router:router_index,..} in self.neighbour_router_iter(i)
			{
				*matrix.get_mut(i,router_index)=1;
			}
		}
		//for(k=0;k<n;k++)
		for k in 0..n
		{
			//for(i=0;i<n;i++)
			for i in 0..n
			{
				//for(j=0;j<n;j++)
				for j in 0..n
				{
					//x=matrix->get(i,k)+matrix->get(k,j);
					let x=*matrix.get(i,k)+*matrix.get(k,j);
					//if(matrix->get(i,j)>x)matrix->get(i,j)=x;
					if *matrix.get(i,j)>x
					{
						*matrix.get_mut(i,j)=x;
					}
				}
			}
		}
		//printf("<<Graph::computeDistanceMatrix\n");
		return matrix;
	}
	
	///Return a pair of matrices `(D,A)` with `D[i,j]` being the distance from `i` to `j`
	///and `A[i,j]` being the number of paths of length `D[i,j]` from `i` to `j`.
	fn compute_amount_shortest_paths(&self) -> (Matrix<usize>,Matrix<usize>)
	{
		//Copied from discrete topologies
		//if(amountMinimumPathsMatrix)return;
		//vertex_index n=size();
		let n=self.num_routers();
		//if(distanceMatrix==NULL)
		//{
		//	distanceMatrix=new Matrix<length>(n,n);
		//}
		let maximum_length=<usize>::MAX/3;
		let mut distance_matrix=Matrix::constant(maximum_length,n,n);
		let mut amount_matrix=Matrix::constant(1,n,n);
		//amountMinimumPathsMatrix=new Matrix<long>(n,n);
		//for(long i=0;i<n;i++)
		//for(long j=0;j<n;j++)
		//{
		//	distanceMatrix->get(i,j)=LENGTH_MAX;
		//	amountMinimumPathsMatrix->get(i,j)=1;
		//}
		//for(vertex_index origin=0;origin<n;origin++)
		for origin in 0..n
		{
			//distanceMatrix->get(origin,origin)=0;
			*distance_matrix.get_mut(origin,origin)=0;
			//std::vector<vertex_index> queue(n);
			let mut queue=vec![0;n];
			//long queue_read_index=0, queue_write_index=1;
			let mut queue_read_index=0;
			let mut queue_write_index=1;
			queue[0]=origin;
			while queue_read_index<n
			{
				//vertex_index best=queue[queue_read_index++];
				let best=queue[queue_read_index];
				queue_read_index+=1;
				//std::vector<vertex_index> nbor=neighbours(best);
				//let degree=self.degree(best);
				//length bd=distanceMatrix->get(origin,best);
				let bd=*distance_matrix.get(origin,best);
				//length alt=bd+1;
				let alt=bd+1;
				//long ba=amountMinimumPathsMatrix->get(origin,best);
				let ba=*amount_matrix.get(origin,best);
				//for(std::vector<vertex_index>::iterator it=nbor.begin();it!=nbor.end();++it)
				//for i in 0..degree
				//{
				//	match self.neighbour(best,i).0
				//	{
				//		Location::RouterPort{
				//			router_index,
				//			router_port: _,
				//		} =>
				//		{
				//			//length old=distanceMatrix->get(origin,*it);
				//			let old=*distance_matrix.get(origin,router_index);
				//			if alt<old
				//			{
				//				*distance_matrix.get_mut(origin,router_index)=alt;
				//				*amount_matrix.get_mut(origin,router_index)=ba;
				//				queue[queue_write_index]=router_index;
				//				queue_write_index+=1;
				//			}
				//			else if alt==old
				//			{
				//				//amountMinimumPathsMatrix->get(origin,*it)+=ba;
				//				*amount_matrix.get_mut(origin,router_index)+=ba;
				//			}
				//		}
				//		_ => panic!("what?"),
				//	}
				//}
				for NeighbourRouterIteratorItem{neighbour_router:router_index,..} in self.neighbour_router_iter(best)
				{
					let old=*distance_matrix.get(origin,router_index);
					if alt<old
					{
						*distance_matrix.get_mut(origin,router_index)=alt;
						*amount_matrix.get_mut(origin,router_index)=ba;
						queue[queue_write_index]=router_index;
						queue_write_index+=1;
					}
					else if alt==old
					{
						*amount_matrix.get_mut(origin,router_index)+=ba;
					}
				}
			}
		}
		(distance_matrix,amount_matrix)
	}

	/// Find the components of the subtopology induced via the allowed links.
	/// Returns vector `ret` with `ret[k]` containing the vertices in the `k`-th component.
	fn components(&self,allowed_classes:&[bool]) -> Vec<Vec<usize>>
	{
		let mut r=vec![];
		let n=self.num_routers();
		let mut found=vec![false;n];
		let weights:Vec<usize>=allowed_classes.iter().map(|a|if *a{1}else {<usize>::MAX}).collect();
		for i in 0..n
		{
			if ! found[i]
			{
				let rindex=r.len();
				r.push(vec![i]);
				let d=self.bfs(i,Some(&weights));
				for j in 0..n
				{
					if i!=j && d[j]!=<usize>::MAX
					{
						r[rindex].push(j);
						found[j]=true;
					}
				}
				//println!("Computed component[{}]={:?}",rindex,r[rindex]);
				//println!("Distances({})={:?}",i,d.iter().map(|v|if *v>100{100}else {*v}).collect::<Vec<usize>>());
			}
		}
		return r;
	}
	
	/// returns a couple matrices `(N,F)` with
	///	`N[u,v]` = number of neighbours w of v with `D(u,v)>D(u,w)`.
	///	`F[u,v]` = number of neighbours w of v with `D(u,v)<D(u,w)`.
	/// A router `v` with `F[u,v]=0` is called a boundary vertex of u.
	fn compute_near_far_matrices(&self) -> (Matrix<usize>,Matrix<usize>)
	{
		let n=self.num_routers();
		let mut near_matrix=Matrix::constant(0,n,n);
		let mut far_matrix=Matrix::constant(0,n,n);
		for origin in 0..n
		{
			//  It may be faster with a tuned BFS.
			//let d=self.bfs(i,class_weight);
			//for j in 0..n
			//{
			//	*matrix.get_mut(i,j)=d[j];
			//}
			// But we just check the distance function.
			for target in 0..n
			{
				//let degree=self.degree(target);
				//for index in 0..degree
				//{
				//	let dist = self.distance(origin,target);
				//	match self.neighbour(target,index)
				//	{
				//		(Location::RouterPort{
				//			router_index: w,
				//			router_port: _,
				//		},_link_class) =>
				//		{
				//			let alt = self.distance(origin,w);
				//			if alt>dist
				//			{
				//				*far_matrix.get_mut(origin,target) += 1;
				//			}
				//			else if alt<dist
				//			{
				//				*near_matrix.get_mut(origin,target) += 1;
				//			}
				//		},
				//		(Location::None,_link_class) => continue,//ignore disconnected ports
				//		_ => panic!("what?"),
				//	}
				//}
				let dist = self.distance(origin,target);
				for NeighbourRouterIteratorItem{neighbour_router:w,..} in self.neighbour_router_iter(target)
				{
					let alt = self.distance(origin,w);
					if alt>dist
					{
						*far_matrix.get_mut(origin,target) += 1;
					}
					else if alt<dist
					{
						*near_matrix.get_mut(origin,target) += 1;
					}
				}
			}
		}
		return (near_matrix,far_matrix);
	}
	
	///Computes the eccentricy of a router. That is, the greatest possible length of a shortest path from that router to any other.
	fn eccentricity(&self, router_index:usize) -> usize
	{
		let n=self.num_routers();
		(0..n).map(|other|self.distance(router_index,other)).max().expect("should have a maximum.")
	}

	///Check pairs (port,vc) with
	/// * non-matching endpoint (this is, going backwards a wire you should return to the same router/server)
	/// * breaking the servers-last rule
	/// * optionally check that the link class is within bounds.
	fn check_adjacency_consistency(&self,amount_link_classes: Option<usize>)
	{
		let n=self.num_routers();
		let mut max_link_class=0;
		let min_deg= self.minimum_degree();
		let max_deg= self.maximum_degree();
		for router_index in 0..n
		{
			let deg = self.degree(router_index);
			let mut router_port_count = 0;
			for port_index in 0..self.ports(router_index)
			{
				let (neighbour_location, link_class) = self.neighbour(router_index, port_index);
				if let Some(bound) = amount_link_classes
				{
					assert!(link_class<bound,"link class {} out of bound {} for port {} of router {}",link_class,bound,port_index,router_index);
				}
				if link_class>max_link_class
				{
					max_link_class=link_class;
				}
				match neighbour_location
				{
					Location::RouterPort{
						router_index: neighbour_router,
						router_port: neighbour_port,
					} =>
					{
						router_port_count += 1;
						if let Some(bound) = amount_link_classes
						{
							if link_class+1==bound
							{
								println!("WARNING: using last link class ({}) for a router to router link.",link_class);
							}
						}
						let (rev_location, rev_link_class) = self.neighbour(neighbour_router, neighbour_port);
						match rev_location
						{
							Location::RouterPort{
								router_index: rev_router,
								router_port: rev_port,
							} =>
							{
								if router_index!=rev_router || port_index!=rev_port
								{
									panic!("Non-matching port ({},{}) to ({},{}) non-returns to ({},{}).",router_index,port_index,neighbour_router,neighbour_port,rev_router,rev_port);
								}
							},
							_ =>{
								println!("WARNING: port {} at router {} connects to another router and it is not returned.",port_index,router_index);
								panic!("It does not even return to a router");
							},
						};
						if link_class!=rev_link_class
						{
							panic!("port {} at router {} has non-matching link class {} vs {}",port_index,router_index,link_class,rev_link_class);
						}
						if port_index>=max_deg
						{
							println!("WARNING: port {} at router {} connects to another router and it is >=maximum_degree={}>=degree={}",port_index,router_index,max_deg,deg);
						}
					},
					Location::ServerPort(server_index) =>
					{
						let (rev_location, rev_link_class) = self.server_neighbour(server_index);
						match rev_location
						{
							Location::RouterPort{
								router_index: rev_router,
								router_port: rev_port,
							} =>
							{
								if router_index!=rev_router || port_index!=rev_port
								{
									panic!("Non-matching port ({},{}) to server {} non-returns to ({},{}).",router_index,port_index,server_index,rev_router,rev_port);
								}
							},
							_ => panic!("It does not even return to a router"),
						};
						if link_class!=rev_link_class
						{
							panic!("port {} at router {} has non-matching link class {} vs {}",port_index,router_index,link_class,rev_link_class);
						}
						if port_index<min_deg
						{
							panic!("port {} at router {} connects to a server and it is <minimum_degree={}<=degree={}",port_index,router_index,min_deg,deg);
						}
					},
					Location::None => println!("WARNING: disconnected port {} at router {}",port_index,router_index),
				}
			}
			if router_port_count != deg {
				panic!("Reported degree {deg} for router {router} when {count} neighbours have been found.",deg=deg,router=router_index,count=router_port_count);
			}
			if deg > max_deg {
				panic!("The degree (actual and measured) {deg} for router {router} is greater than reported maximum {max}.",deg=deg,router=router_index,max=max_deg);
			}
			if deg < min_deg {
				panic!("The degree (actual and measured) {deg} for router {router} is lower than reported minimum {min}.",deg=deg,router=router_index,min=min_deg);
			}
			if deg==0 {
				println!("WARNING: *** router {} has no link to other routers!! ***",router_index);
			}
		}
		if let Some(bound)=amount_link_classes
		{
			if bound!=max_link_class+1
			{
				println!("WARNING: querying {} link classes when the topology has {}",bound,max_link_class+1);
			}
		}
	}
	///Dump the adjacencies into a file.
	///You may use NeighboursLists::file_adj to load them.
	fn write_adjacencies_to_file(&self, file:&mut File, _format:usize)->Result<(),std::io::Error>
	{
		let n=self.num_routers();
		writeln!(file,"NODOS {}",n)?;
		writeln!(file,"GRADO {}",self.maximum_degree())?;
		//for (router_index,neighbour_list) in self.list.iter().enumerate()
		for router_index in 0..n
		{
			writeln!(file,"N {}",router_index)?;
			let neighbour_string=self.neighbour_router_iter(router_index).map(|item|item.neighbour_router.to_string()).collect::<Vec<String>>().join(" ");
			writeln!(file,"{}",neighbour_string)?;
		}
		Ok(())
	}
}

//#[non_exhaustive]
///The use may want to build topologies himself, and it cannot be `Default' unless we move to `Cow'. So I am removing the non_exhaustive attribute.
pub struct TopologyBuilderArgument<'a>
{
	///A ConfigurationValue::Object defining the topology.
	pub cv: &'a ConfigurationValue,
	///The user defined plugs. In case the topology needs to create elements.
	pub plugs: &'a Plugs,
	///The random number generator to use.
	pub rng: &'a mut StdRng,
}

impl<'a> TopologyBuilderArgument<'a>
{
	fn with_cv<'b>(self:&'b mut TopologyBuilderArgument<'a>, new_cv: &'b ConfigurationValue) -> TopologyBuilderArgument<'b>
	{
		TopologyBuilderArgument{
			cv: new_cv,
			plugs: self.plugs,
			rng: self.rng,
		}
	}
}

/**
Build a topology. All topologies should admit an optional `legend_name` to be used in plots.

## Cartesian topologies

### Mesh example
A bidimensional [mesh](Mesh) of side 16. Routers in the periphery has less degree, defined as un-connected ports.
```ignore
Mesh{
	sides: [16,16],
	servers_per_router:1,
	legend_name: "A 16x16 mesh network",
}
```

### Torus example
A bidimensional [torus](Torus) of side 16. All routers have degree 4. Plus another port to connect to the server.
```ignore
Torus{
	sides: [16,16],
	servers_per_router:1,
	legend_name: "A 16x16 torus network",
}
```

### Hamming example
A bidimensional [Hamming] graph isomorphic to the Cartesian product of two Complete graph of 16 vertices. Also known as HyperX, flattened butterfly topology, generalized hypercube, or rook graph. Has degree 2*(16-1)=30. It is recommended to use a number of servers per router close to the side value.
```ignore
Hamming{
	sides: [16,16],
	servers_per_router:16,
	legend_name: "A 16x16 Hamming network",
}
```


## Topologies given by lists of neighbours.

### Random regular graph example
A [random regular graph](NeighboursLists) can be built when at least one of `degree` or `routers` is an even number. A useful formula is `degree^k=2routers*ln(routers)`, where the exponent `k` is close to the average distance. For large enough numbers `ceil(k)` should be the diameter. To have enough population `severs_per_router` should be a little below the quotient `degree/average_distance`, as some little throughput is wasted by the non-uniforme use of the links.
```ignore
RandomRegularGraph{
	routers: 500,
	degree: 20,
	servers_per_router: 8,
	legend_name: "A random 20-regular graph of 500 routers",
}
```

### File example
A [file](NeighboursLists) can be load as topology. This can be useful to keep a specific random graph without need to care about using the same RNG seed. It can also be used to simulate topologies generated by other software.
```ignore
File{
	filename: "/path/to/my/topology/file",
	format: 0,//TODO: this needs documentation...
	servers_per_router: 5,
	legend_name: "some network in the device",
}
```

## Dragonfly networks.
The `global_ports_per_router` was denotated `h` in the original article of the [Dragonfly].
The number of servers per router can be varied, but recommended to the same value as `global_ports_per_router`.
By default use the palm-tree arrangement (see [Palmtree](dragonfly::Palmtree)) of global links,
but it can be changed to a random arrangement (see [RandomArrangement](dragonfly::RandomArrangement)).

See [Arrangement](dragonfly::Arrangement) for more details on the arrangement of global links.

```ignore
// Example with palm-tree arrangement (default)
Dragonfly{
	global_ports_per_router: 4,
	servers_per_router: 4,
	legend_name: "h=4 dragonfly with palm-tree global arrangement",
}

// Example with random arrangement
Dragonfly{
	global_ports_per_router: 4,
	servers_per_router: 4,
	global_arrangement: Random,
	legend_name: "h=4 dragonfly with random global arrangement",
}
```


## Networks built over finite fields. Only prime fields are currently supported.

### LeviProjective.
The [LeviProjective] topology is the Levi graph of the projective plane over a finite field. Both lines and points of the projective plane become vertices, that is, routers. Has average distance around 2.5, diameter 3 and girth 6. The finite field is of order `prime`, that should be a prime number. Powers are not yet supported. The topology degree is the prime plus one. Called projective networks in "Projective Networks: Topologies for Large Parallel Computer Systems" by C. Camarero et al.
```ignore
LeviProjective{
	prime: 19,
	servers_per_router: 8,
	legend_name: "Levi-projective network over q=19",
}
```

### Projective.
The [Projective] topology is the quotient of the LeviProjective over a polarity: a bijection between points and lines that maintains incidence. It is also known as Brown graph or Erdös--Renyí graph. The degree is again `prime+1`, except in the fixed points which became loops. These loops are removed from the network, becoming non-conected ports. Has diameter 2, average distance a little below and girth 5. Called demi-projective networks in "Projective Networks: Topologies for Large Parallel Computer Systems" by C. Camarero et al.
```ignore
Projective{
	prime: 19,
	servers_per_router: 10,
	legend_name: "demi-projective network over q=19",
}
```

### SlimFly.
The [SlimFly] is the MMS (Mirka--Miller--Siran) graph. For `prime=5` it is the Hogffman--Singleton graph.
Has Paley graphs as subgraph, or similar depending on whether the prime is congruent to what modulo 4. has diameter 2.
Note the links in the (quasi)-Paley graph (which we can call local links) are used in a slightly different amount to the other links.
This slightly reduces the delivered throughput.
```ignore
SlimFly{
	prime: 19,
	//primitive: 2,//optional value, should actually be a primitive number. Should be better to let it be calculated.
	servers_per_router:9,
	legend_name: "Slimfly MMS over q=19",
}
```

## Multi-stage networks.
In a multi-stage network routers are grouped by levels, which the routers within a level connecting only to routers of the preceding and the next levels. Routers at level 0 (the first level) are called leaf routers and they are the ones connected servers. We call stages to the connections from a level to the next. The number of stages is called height and it is one less than the number of levels. The levels hence range from 0 up to height (both inclusive).

### Generic MultiStage
See [MultiStage].
```ignore
MultiStage{
	stages:[
		Fat { bottom_factor:4, top_factor:4 },
		Fat { bottom_factor:8, top_factor:4 },
	],
	servers_per_leaf: 4,
	legend_name: "a fat-tree defined using stages"
}
```

### XGFT
See [FatStage](multistage::FatStage) and [MultiStage].
An eXtended Generalized Fat-Tree, see "On Generalized Fat Trees" by S. R. Öhring et al.

```ignore
XGFT{
	height: 3,
	down: [4,4,8],
	up: [4,4,4],
	servers_per_leaf: 4,
	legend_name: "XGFT(3;4,4,8;4,4,4)",
}
```

### OFT
See [ProjectiveStage](multistage::ProjectiveStage) and [MultiStage].
Orthogonal Fat-Tree, see "Recursively Scalable Fat-Trees as Interconnection Networks" by M. Valerio et al. Uses the construction shown in "Projective Networks: Topologies for Large Parallel Computer Systems" by C. Camarero et al. For the moment only implemented for projective planes over prime finite fields, excluding higher powers.

The optional parameter `double_topmost_level` (default to true) indicates whether the bottom of the last stage should be doubled, as using all ports in the topmost routers for downwards connections.

```ignore
OFT{
	height: 2,
	prime: 3,
	servers_per_leaf: 4,
	//double_topmost_level: false,//optional parameter
	legend_name: "OFT over the projective plane of 3 points",
}
```

### RFC
See [ExplicitStage](multistage::ExplicitStage) and [MultiStage].
Random Folded Clos. See "Random Folded Clos Topologies for Datacenter Networks" by C. Camarero et al.

```ignore
RFC{
	height: 3,
	down: [10,10,20],
	up: [10,10,10],
	sizes: [80,80,80,40],
	servers_per_leaf: 4,
	legend_name: "RGC of radix 20 with 80 leaf routers",
}
```

## Operations

### RemappedServersTopology

[RemappedServersTopology](operations::RemappedServersTopology) transforms the server indices of a base topology. This does not change the indices of routers. The pattern is called once
to generate a map from the base servers to the used indices. This resulting map must be a permutation and it would panic otherwise.
The pattern may be the Identity for no change. A RandomPermutation is a shuffle of the server identifiers.

Example configuration:
```ignore
RemappedServers{
	topology: Mesh{sides:[4,4],servers_per_router:1},
	pattern: RandomPermutation,
}
```

## AsCartesianTopology
[AsCartesianTopology] provides a topology with a given representation as a block with Cartesian coordinates.

Example loading a topology with 100 switches and sorting them as a 10 times 10 block.
```ignore
AsCartesianTopology{
	topology: File{ filename:"/path/topo_with_100_switches", format:0, servers_per_router:5 }
	sides: [10,10],
}
```

*/
pub fn new_topology(arg:TopologyBuilderArgument) -> Box<dyn Topology>
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
	{
		if let Some(builder) = arg.plugs.topologies.get(cv_name)
		{
			return builder(arg);
		}
		match cv_name.as_ref()
		{
			"Mesh" => Box::new(Mesh::new(arg.cv)),
			"Torus" => Box::new(Torus::new(arg.cv)),
			"RandomRegularGraph" | "File" => Box::new(NeighboursLists::new_cfg(arg.cv,arg.rng)),
			"Hamming" => Box::new(Hamming::new(arg.cv)),
			"Dragonfly" | "CanonicDragonfly" => Box::new(Dragonfly::new(arg)),
			"Projective" => Box::new(Projective::new(arg)),
			"LeviProjective" => Box::new(LeviProjective::new(arg)),
			"SlimFly" => Box::new(SlimFly::new(arg)),
			"MultiStage" | "XGFT" | "OFT" | "RFC" => Box::new(MultiStage::new(arg)),
			"Megafly" => Box::new(megafly::Megafly::new(arg)),
			"RemappedServers" => Box::new(operations::RemappedServersTopology::new(arg)),
			"AsCartesianTopology" => Box::new(AsCartesianTopology::new(arg)),
			"RandomLinkFaults" => Box::new(operations::RandomLinkFaults::new(arg)),
			_ => panic!("Unknown topology {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create a topology from a non-Object");
	}
}

