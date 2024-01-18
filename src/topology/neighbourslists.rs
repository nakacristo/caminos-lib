
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufRead,BufReader};

use ::rand::{Rng,rngs::StdRng};
use quantifiable_derive::Quantifiable;//the derive macro
use super::{Topology,Location};
use super::cartesian::CartesianData;
use crate::config_parser::ConfigurationValue;
use crate::matrix::Matrix;

///A topology based on having sotred the list of neighbours to each router.
///It is used
///* to load a topology from a file (topology=File)
///* and to create a topology with random links (topology=RandomRegularGraph).
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct NeighboursLists
{
	///`list[router][k]` = `k`-th neighbour of router; router_index + port.
	list: Vec<Vec<(usize,usize)>>,
	///`servers[router]` = number of servers connected to router
	servers: Vec<usize>,

	//Caches.
	///`servers_offsets[router] = s` means the servers s,s+1,... are atached to router
	server_offsets: Vec<usize>,
	///`router_by_server[server] = attached router + port`
	routers_by_server: Vec<(usize,usize)>,
	///distance_matrix.get(i,j) = distance from router i to router j
	distance_matrix:Matrix<usize>,
	///amount_matrix.get(i,j) = amount of shortest paths from router i to router j
	amount_matrix:Matrix<usize>,
	///Average of the amount_matrix entries.
	average_amount: f32,
}

//impl Quantifiable for NeighboursLists
//{
//	fn total_memory(&self) -> usize
//	{
//		return size_of::<NeighboursLists>() + self.list.total_memory() + self.servers.total_memory() + self.server_offsets.total_memory() + self.routers_by_server.total_memory() + self.distance_matrix.total_memory() + self.amount_matrix.total_memory();
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

impl Topology for NeighboursLists
{
	fn num_routers(&self) -> usize
	{
		self.list.len()
	}
	fn num_servers(&self) -> usize
	{
		//self.servers.iter().sum()
		self.routers_by_server.len()
	}
	//fn num_arcs(&self) -> usize
	//{
	//	//self.num_routers()*self.cartesian_data.sides.len()*2
	//	unimplemented!()
	//}
	//fn num_servers(&self, _router_index:usize) -> usize
	//{
	//	self.servers_per_router
	//}
	fn neighbour(&self, router_index:usize, port: usize) -> (Location,usize)
	{
		let degree=self.list[router_index].len();
		if port<degree
		{
			let (r,p) = self.list[router_index][port];
			return (Location::RouterPort{router_index:r,router_port:p},0);
		}
		(Location::ServerPort(self.server_offsets[router_index]+port-degree),1)
	}
	fn server_neighbour(&self, server_index:usize) -> (Location,usize)
	{
		let (r,p) = self.routers_by_server[server_index];
		(Location::RouterPort{router_index:r,router_port:p},1)
	}
	fn diameter(&self) -> usize
	{
		self.compute_diameter()
	}
	fn distance(&self,origin:usize,destination:usize) -> usize
	{
		//unimplemented!();
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
	fn maximum_degree(&self) -> usize
	{
		self.list.iter().map(|adj|adj.len()).max().expect("calling maximum_degree without routers")
	}
	fn minimum_degree(&self) -> usize
	{
		self.list.iter().map(|adj|adj.len()).min().expect("calling minimum_degree without routers")
	}
	fn degree(&self, router_index: usize) -> usize
	{
		self.list[router_index].len()
	}
	fn ports(&self, router_index: usize) -> usize
	{
		self.list[router_index].len() + self.servers[router_index]
	}
	fn cartesian_data(&self) -> Option<&CartesianData>
	{
		None
	}
	fn coordinated_routing_record(&self, _coordinates_a:&[usize], _coordinates_b:&[usize], _rng: Option<&mut StdRng>)->Vec<i32>
	{
		//(0..coordinates_a.len()).map(|i|coordinates_b[i] as i32-coordinates_a[i] as i32).collect()
		unimplemented!();
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

impl NeighboursLists
{
	///Build a topology with the given list of adjacency for routers and servers.
	///`list[i][j] = (r,p)` with `r` the `j`-th neighbour router of the `i`-th router, where `i` is the `p`-th neighbour of `r`.
	///`servers[i]` = amount of servers connected to the `i`-th router.
	pub fn new(list:Vec<Vec<(usize,usize)>>,servers:Vec<usize>) -> NeighboursLists
	{
		let mut server_offsets=Vec::with_capacity(servers.len());
		let mut offset=0;
		for &size in servers.iter()
		{
			server_offsets.push(offset);
			offset+=size;
		}
		let mut routers_by_server=Vec::with_capacity(offset);
		for (router_index,&size) in servers.iter().enumerate()
		{
			let degree=list[router_index].len();
			for i in 0..size
			{
				routers_by_server.push((router_index,degree+i));
			}
		}
		//println!("offset={} routers_by_server.len()={}",offset,routers_by_server.len());
		let mut topo=NeighboursLists{
			list,
			servers,
			server_offsets,
			routers_by_server,
			distance_matrix:Matrix::constant(0,0,0),
			amount_matrix:Matrix::constant(0,0,0),
			average_amount: 0f32,
		};
		//topo.distance_matrix=topo.compute_distance_matrix();
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
	///Build random regular adjacencies.
	pub fn new_rrg_adj(routers:usize, degree:usize, rng: &mut StdRng) -> Vec<Vec<usize>>
	{
		//long U[routers*degree];//available
		//std::vector<std::vector<vertex_index> > adj(routers);
		//let mut adj=vec![BTreeSet::new();routers];
		let mut adj=vec![Vec::with_capacity(degree);routers];
		let mut go=true;
		while go
		{
			go=false;
			#[allow(non_snake_case)]
			let mut Un=routers*degree;
			//for(i=0;i<routers*degree;i++)U[i]=i;
			#[allow(non_snake_case)]
			let mut U=(0..routers*degree).collect::<Vec<usize>>();
			//for(i=0;i<routers;i++)adj[i].clear();
			for adjs in adj.iter_mut()
			{
				adjs.clear();
			}
			//std::set<vertex_index> A;
			//for(i=0;i<routers;i++)A.insert(i);
			#[allow(non_snake_case)]
			let mut A=(0..routers).collect::<BTreeSet<usize>>();
			while Un>0
			{
				if A.len()<degree
				{
					let mut good=false;
					//for(vertex_index i:A)for(vertex_index j:A)
					for &i in A.iter()
					{
						for &j in A.iter()
						{
							if j<=i
							{
								continue;
							}
							let mut inadj=false;
							//for(k=0;k<adj[j].size();k++)if(adj[j][k]==i)
							for &neigh in adj[j].iter()
							{
								if neigh==i
								{
									inadj=true;
									break;
								}
							}
							if !inadj
							{
								good=true;
							}
						}
					}
					if !good
					{
						go=true;
						break;
					}
				}
				//sample points x,y, keep them last in U to remove them in O(1)
				//vertex_index r=randomInteger(Un);
				//let r=rng.gen_range(0,Un);//rand-0.4
				let r=rng.gen_range(0..Un);//rand-0.8
				//vertex_index x=U[r];
				let x=U[r];
				U[r]=U[Un-1];
				U[Un-1]=x;

				//r=randomInteger(Un-1);
				let r=rng.gen_range(0..Un-1);
				//vertex_index y=U[r];
				let y=U[r];
				U[r]=U[Un-2];
				U[Un-2]=y;

				//vertex_index u=x/degree, v=y/degree;//vertices
				let u=x/degree;
				let v=y/degree;
				if u==v
				{
					continue;//no loops
				}
				let mut inadj=false;
				//for(i=0;i<adj[u].size();i++)if(adj[u][i]==v)
				for &neigh in adj[u].iter()
				{
					if neigh==v
					{
						inadj=true;
						break;
					}
				}
				if inadj
				{
					continue;//no multiple edges
				}
				//printf("adding edge %ld -- %ld\n",u,v);
				Un-=2;
				adj[u].push(v);
				if adj[u].len()==degree
				{
					A.remove(&u);
				}
				adj[v].push(u);
				if adj[v].len()==degree
				{
					A.remove(&v);
				}
			}
		}
		adj
	}
	///Get the adjancecies from a given file.
	pub fn file_adj(file:&File, _format:usize) -> Vec<Vec<usize>>
	{
		//let mut adj=vec![Vec::with_capacity(degree);routers];
		let mut adj : Vec<Vec<usize>> =vec![];
		let mut nodos=None;
		let reader = BufReader::new(file);
		let mut lines=reader.lines();
		//for rline in reader.lines()
		while let Some(rline)=lines.next()
		{
			let line=rline.expect("Some problem when reading the topology.");
			//println!("line: {}",line);
			let mut words=line.split_whitespace();
			match words.next()
			{
				Some("NODOS") =>
				{
					nodos=Some(words.next().unwrap().parse::<usize>().unwrap());
				},
				Some("GRADO") =>
				{
					let grado=Some(words.next().unwrap().parse::<usize>().unwrap());
					if let Some(routers)=nodos
					{
						if let Some(degree)=grado
						{
							adj=vec![Vec::with_capacity(degree);routers];
						}
					}
				},
				Some("N") =>
				{
					let current=words.next().unwrap().parse::<usize>().unwrap();
					for wneighbour in lines.next().unwrap().unwrap().split_whitespace()
					{
						let neighbour=wneighbour.parse::<usize>().unwrap();
						adj[current].push(neighbour);
					}
				},
				_ => panic!("Illegal word"),
			};
		}
		adj
	}
	///Build a new NeighboursLists from a ConfigurationValue.
	/// * severs_per_router
	/// * legend_name: optionally for generating output.
	///File topologies use
	/// * filename: for importing from a file
	/// * format: format of the improted filename.
	///RandomRegularGraph topologies use
	/// * routers: the total number of routers.
	/// * degree: the degree, ports towards other routers.
	pub fn new_cfg(cv:&ConfigurationValue, rng: &mut StdRng) -> NeighboursLists
	{
		let mut routers=None;
		let mut degree=None;
		let mut servers_per_router=None;
		let mut filename=None;
		let mut format=None;
		enum Kind { RandomRegularGraph, File }
		let kind;
		if let &ConfigurationValue::Object(ref cv_name, ref cv_pairs)=cv
		{
			//if cv_name!="RandomRegularGraph"
			//{
			//	panic!("A RandomRegularGraph must be created from a `RandomRegularGraph` object not `{}`",cv_name);
			//}
			kind=match cv_name.as_ref()
			{
				"RandomRegularGraph" => Kind::RandomRegularGraph,
				"File" => Kind::File,
				_ => panic!("Unknown topology {}",cv_name),
			};
			for &(ref name,ref value) in cv_pairs
			{
				match name.as_ref()
				{
					"routers" => match value
					{
						&ConfigurationValue::Number(f) => routers=Some(f as usize),
						_ => panic!("bad value for routers"),
					},
					"degree" => match value
					{
						&ConfigurationValue::Number(f) => degree=Some(f as usize),
						_ => panic!("bad value for degree"),
					},
					"servers_per_router" => match value
					{
						&ConfigurationValue::Number(f) => servers_per_router=Some(f as usize),
						_ => panic!("bad value for servers_per_router"),
					},
					"filename" => match value
					{
						&ConfigurationValue::Literal(ref s) => filename=Some(s.to_string()),
						_ => panic!("bad value for filename"),
					},
					"format" => match value
					{
						&ConfigurationValue::Number(f) => format=Some(f as usize),
						_ => panic!("bad value for format"),
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
		let servers_per_router=servers_per_router.expect("There were no servers_per_router");

		let adj = match kind
		{
			Kind::RandomRegularGraph =>
			{
				let routers=routers.expect("There were no routers");
				let degree=degree.expect("There were no degree");
				Self::new_rrg_adj(routers,degree,rng)
			},
			Kind::File =>
			{
				let filename=filename.expect("There were no filename");
				let format=format.expect("There were no format");
				let file=File::open(&filename).expect("could not open topology file.");
				Self::file_adj(&file,format)
			},
		};
		//return new NeighboursLists(adj);
		let list=adj.iter().enumerate().map(|(current,neighbours)|
			neighbours.iter().map(|&neigh|(neigh,
			{
				let mut index=0;
				for (i,&v) in adj[neigh].iter().enumerate()
				{
					if v==current
					{
						index=i;
						break;
					}
				}
				index
			})).collect()
		).collect();
		//let servers=vec![servers_per_router;routers];
		let servers=vec![servers_per_router;adj.len()];
		NeighboursLists::new(list,servers)
	}
}

