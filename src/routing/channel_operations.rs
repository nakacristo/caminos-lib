/*!

Implementations of operations on routings that modify the virtual channel to use.

* ChannelsPerHop
* ChannelsPerHopPerLinkClass
* ChannelMap
* AscendantChannelsWithLinkClass

*/

use std::cell::RefCell;

use ::rand::{rngs::StdRng};
use rand::SeedableRng;

use crate::{match_object_panic, Plugs};
use crate::config_parser::ConfigurationValue;
use crate::pattern::{new_pattern, PatternBuilderArgument};
use crate::topology::{new_topology, Topology, TopologyBuilderArgument};
use crate::routing::prelude::*;

///Set the virtual channels to use in each hop.
///Sometimes the same can be achieved by the router policy `Hops`.
#[derive(Debug)]
pub struct ChannelsPerHop
{
	///The base routing to use.
	routing: Box<dyn Routing>,
	///`channels[k]` is the list of available VCs to use in the `k`-th hop.
	///This includes the last hop towards the server.
	channels: Vec<Vec<usize>>,
}

impl Routing for ChannelsPerHop
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//println!("{}",topology.diameter());
		let vcs = &self.channels[routing_info.hops];
		let candidates = self.routing.next(routing_info,topology,current_router,target_router,target_server,num_virtual_channels,rng)?;
		let idempotent = candidates.idempotent;
		let r = candidates.into_iter().filter(|c|vcs.contains(&c.virtual_channel)).collect();
		Ok(RoutingNextCandidates{candidates:r,idempotent})
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		self.routing.initialize_routing_info(routing_info,topology,current_router,target_router,target_server,rng);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		self.routing.update_routing_info(routing_info,topology,current_router,current_port,target_router,target_server,rng);
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.routing.initialize(topology,rng);
	}
	fn performed_request(&self, requested:&CandidateEgress, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, num_virtual_channels:usize, rng:&mut StdRng)
	{
		self.routing.performed_request(requested,routing_info,topology,current_router,target_router,target_server,num_virtual_channels,rng);
	}
	fn statistics(&self, cycle:Time) -> Option<ConfigurationValue>
	{
		self.routing.statistics(cycle)
	}
	fn reset_statistics(&mut self, next_cycle:Time)
	{
		self.routing.reset_statistics(next_cycle)
	}
}

impl ChannelsPerHop
{
	pub fn new(arg: RoutingBuilderArgument) -> ChannelsPerHop
	{
		let mut routing =None;
		let mut channels =None;
		match_object_panic!(arg.cv,"ChannelsPerHop",value,
			"routing" => routing=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"channels" => channels=Some(value.as_array().expect("bad value in channels").iter()
				.map(|vcs_this_hop| vcs_this_hop.as_array().expect("bad value in channels").iter()
					.map(|vc| vc.as_f64().expect("bad value in channels") as usize).collect()
				).collect()
			),
		);
		let routing=routing.expect("There were no routing");
		let channels=channels.expect("There were no channels");
		ChannelsPerHop{
			routing,
			channels,
		}
	}
}

///Set the virtual channels to use in each hop for each link class.
///See also the simpler transformation by ChannelsPerHop.
#[derive(Debug)]
pub struct ChannelsPerHopPerLinkClass
{
	///The base routing to use.
	routing: Box<dyn Routing>,
	///`channels[class][k]` is the list of available VCs to use in the k-th hop given in links of the given `class` if `use_total_hops` is false
	/// or the absolute k-th hop when it is true..
	channels: Vec<Vec<Vec<usize>>>,
	///Use total number of hops for each link class.
	use_total_hops: bool,
}

impl Routing for ChannelsPerHopPerLinkClass
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//println!("{}",topology.diameter());
		let candidates = self.routing.next(&routing_info.meta.as_ref().unwrap()[0].borrow(),topology,current_router,target_router,target_server,num_virtual_channels,rng)?;
		let idempotent = candidates.idempotent;
		let hops = &routing_info.selections.as_ref().unwrap();
		let r = candidates.into_iter().filter(|c|{
			let (_next_location,link_class)=topology.neighbour(current_router,c.port);
			let h = if self.use_total_hops
			{
				routing_info.hops
			}else{
				hops[link_class] as usize
			};

			//println!("h={} link_class={} channels={:?}",h,link_class,self.channels[link_class]);
			if self.channels[link_class].len()<=h
			{
				panic!("Already given {} hops by link class {}",h,link_class);
			}
			//self.channels[link_class].len()>h && self.channels[link_class][h].contains(&c.virtual_channel)
			self.channels[link_class][h].contains(&c.virtual_channel)
		}).collect();
		Ok(RoutingNextCandidates{candidates:r,idempotent})
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		let mut info = routing_info.borrow_mut();
		info.meta=Some(vec![ RefCell::new(RoutingInfo::new())]);
		info.selections = Some(vec![0;self.channels.len()]);
		self.routing.initialize_routing_info(&info.meta.as_ref().unwrap()[0],topology,current_router,target_router,target_server,rng);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		let (_previous_location,link_class)=topology.neighbour(current_router,current_port);
		let mut info = routing_info.borrow_mut();
		if let Some(ref mut hops)=info.selections
		{
			if hops.len() <= link_class
			{
				println!("WARNING: In ChannelsPerHopPerLinkClass, {} classes where not enough, hop through class {}",hops.len(),link_class);
				hops.resize(link_class+1,0);
			}
			hops[link_class] += 1;
		}
		let subinfo = &info.meta.as_ref().unwrap()[0];
		subinfo.borrow_mut().hops+=1;
		self.routing.update_routing_info(subinfo,topology,current_router,current_port,target_router,target_server,rng);
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.routing.initialize(topology,rng);
	}
	fn performed_request(&self, requested:&CandidateEgress, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, num_virtual_channels:usize, rng:&mut StdRng)
	{
		self.routing.performed_request(requested,&routing_info.borrow().meta.as_ref().unwrap()[0],topology,current_router,target_router,target_server,num_virtual_channels,rng);
	}
	fn statistics(&self, cycle:Time) -> Option<ConfigurationValue>
	{
		self.routing.statistics(cycle)
	}
	fn reset_statistics(&mut self, next_cycle:Time)
	{
		self.routing.reset_statistics(next_cycle)
	}
}

impl ChannelsPerHopPerLinkClass
{
	pub fn new(arg: RoutingBuilderArgument) -> ChannelsPerHopPerLinkClass
	{
		let mut routing =None;
		let mut channels =None;
		let mut use_total_hops = false;
		match_object_panic!(arg.cv,"ChannelsPerHopPerLinkClass",value,
			"routing" => routing=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"use_total_hops" => use_total_hops=match value
            {
				&ConfigurationValue::True => true,
				&ConfigurationValue::False => false,
                _ => panic!("bad value for use_total_hops"),
            },
			"channels" => match value
			{
				&ConfigurationValue::Array(ref classlist) => channels=Some(classlist.iter().map(|v|match v{
					&ConfigurationValue::Array(ref hoplist) => hoplist.iter().map(|v|match v{
						&ConfigurationValue::Array(ref vcs) => vcs.iter().map(|v|match v{
							&ConfigurationValue::Number(f) => f as usize,
							_ => panic!("bad value in channels"),
						}).collect(),
						_ => panic!("bad value in channels"),
					}).collect(),
					_ => panic!("bad value in channels"),
				}).collect()),
				_ => panic!("bad value for channels"),
			}
		);
		let routing=routing.expect("There were no routing");
		let channels=channels.expect("There were no channels");
		ChannelsPerHopPerLinkClass{
			routing,
			channels,
			use_total_hops,
		}
	}
}

#[derive(Debug)]
pub struct AscendantChannelsWithLinkClass
{
	///The base routing to use.
	routing: Box<dyn Routing>,
	bases: Vec<usize>,
}

impl Routing for AscendantChannelsWithLinkClass
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//println!("{}",topology.diameter());
		let candidates = self.routing.next(&routing_info.meta.as_ref().unwrap()[0].borrow(),topology,current_router,target_router,target_server,num_virtual_channels,rng)?;
		let idempotent = candidates.idempotent;
		let hops_since = &routing_info.selections.as_ref().unwrap();
		let r = candidates.into_iter().filter(|c|{
			let (_next_location,link_class)=topology.neighbour(current_router,c.port);
			if link_class>= self.bases.len() { return true; }
			//let h = hops_since[link_class] as usize;
			let vc = (link_class..self.bases.len()).rev().fold(0, |x,class| x*self.bases[class]+(hops_since[class] as usize) );
			//if link_class==0 && vc!=hops_since[1] as usize{ println!("hops_since={:?} link_class={} vc={}",hops_since,link_class,vc); }
			c.virtual_channel == vc
		}).collect();
		Ok(RoutingNextCandidates{candidates:r,idempotent})
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		let mut info = routing_info.borrow_mut();
		info.meta=Some(vec![ RefCell::new(RoutingInfo::new())]);
		info.selections = Some(vec![0;self.bases.len()]);
		self.routing.initialize_routing_info(&info.meta.as_ref().unwrap()[0],topology,current_router,target_router,target_server,rng);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		let (_previous_location,link_class)=topology.neighbour(current_router,current_port);
		let mut info = routing_info.borrow_mut();
		if let Some(ref mut hops_since)=info.selections
		{
			if hops_since.len() <= link_class
			{
				println!("WARNING: In AscendantChannelsWithLinkClass, {} classes where not enough, hop through class {}",hops_since.len(),link_class);
				hops_since.resize(link_class+1,0);
			}
			hops_since[link_class] += 1;
			for x in hops_since[0..link_class].iter_mut()
			{
				*x=0;
			}
		}
		let subinfo = &info.meta.as_ref().unwrap()[0];
		subinfo.borrow_mut().hops+=1;
		self.routing.update_routing_info(subinfo,topology,current_router,current_port,target_router,target_server,rng);
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.routing.initialize(topology,rng);
	}
	fn performed_request(&self, requested:&CandidateEgress, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, num_virtual_channels:usize, rng:&mut StdRng)
	{
		self.routing.performed_request(requested,&routing_info.borrow().meta.as_ref().unwrap()[0],topology,current_router,target_router,target_server,num_virtual_channels,rng);
	}
	fn statistics(&self, cycle:Time) -> Option<ConfigurationValue>
	{
		self.routing.statistics(cycle)
	}
	fn reset_statistics(&mut self, next_cycle:Time)
	{
		self.routing.reset_statistics(next_cycle)
	}
}

impl AscendantChannelsWithLinkClass
{
	pub fn new(arg: RoutingBuilderArgument) -> AscendantChannelsWithLinkClass
	{
		let mut routing =None;
		let mut bases =None;
		match_object_panic!(arg.cv,"AscendantChannelsWithLinkClass",value,
			"routing" => routing=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"bases" => bases = Some(value.as_array()
				.expect("bad value for bases").iter()
				.map(|v|v.as_f64().expect("bad value in bases") as usize).collect()),
		);
		let routing=routing.expect("There were no routing");
		let bases=bases.expect("There were no bases");
		AscendantChannelsWithLinkClass{
			routing,
			bases,
		}
	}
}

///Just remap the virtual channels.
#[derive(Debug)]
pub struct ChannelMap
{
	///The base routing to use.
	routing: Box<dyn Routing>,
	map: Vec<Vec<usize>>,
}

impl Routing for ChannelMap
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, _num_virtual_channels:usize, rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//println!("{}",topology.diameter());
		//let vcs = &self.channels[routing_info.hops];
		let candidates = self.routing.next(routing_info,topology,current_router,target_router,target_server,self.map.len(),rng)?;
		let idempotent = candidates.idempotent;
		//candidates.into_iter().filter(|c|vcs.contains(&c.virtual_channel)).collect()
		let mut r=Vec::with_capacity(candidates.len());
		for can in candidates.into_iter()
		{
			for vc in self.map[can.virtual_channel].iter()
			{
				let mut new = can.clone();
				new.virtual_channel = *vc;
				r.push(new);
			}
		}
		Ok(RoutingNextCandidates{candidates:r,idempotent})
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		self.routing.initialize_routing_info(routing_info,topology,current_router,target_router,target_server,rng);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		self.routing.update_routing_info(routing_info,topology,current_router,current_port,target_router,target_server,rng);
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng)
	{


		self.routing.initialize(topology,rng);
	}
	fn performed_request(&self, requested:&CandidateEgress, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, _num_virtual_channels:usize, rng:&mut StdRng)
	{
		self.routing.performed_request(requested,routing_info,topology,current_router,target_router,target_server,self.map.len(),rng);
	}
	fn statistics(&self, cycle:Time) -> Option<ConfigurationValue>
	{
		self.routing.statistics(cycle)
	}
	fn reset_statistics(&mut self, next_cycle:Time)
	{
		self.routing.reset_statistics(next_cycle)
	}
}

impl ChannelMap
{
	pub fn new(arg: RoutingBuilderArgument) -> ChannelMap
	{
		let mut routing =None;
		let mut map =None;
		let mut physical_to_logical =None; //pattern
		let mut logical_size = None; //the size of the logical channels
		let mut physical_size = None; //the size of the physical channels
		match_object_panic!(arg.cv,"ChannelMap",value,
			"routing" => routing=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"map" => match value
			{
				&ConfigurationValue::Array(ref hoplist) => map=Some(hoplist.iter().map(|v|match v{
					&ConfigurationValue::Array(ref vcs) => vcs.iter().map(|v|match v{
						&ConfigurationValue::Number(f) => f as usize,
						_ => panic!("bad value in map"),
					}).collect(),
					_ => panic!("bad value in map"),
				}).collect()),
				_ => panic!("bad value for map"),
			},
			"physical_to_logical" => physical_to_logical = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"logical_size" => logical_size = Some(value.as_f64().expect("bad value for logical_size") as usize),
			"physical_size" => physical_size = Some(value.as_f64().expect("bad value for physical_size") as usize),
		);

		let map = map.unwrap_or_else(||{
			//TODO: clearly we need some constructors for internal use that do not use plugs.
			let plugs = Plugs::default();
			let mut rng=StdRng::seed_from_u64(10u64);
			let topo_cv = ConfigurationValue::Object("Hamming".to_string(),vec![("sides".to_string(),ConfigurationValue::Array(vec![])), ("servers_per_router".to_string(),ConfigurationValue::Number(1.0))]);

			let dummy_topology = new_topology(TopologyBuilderArgument{cv:&topo_cv,plugs:&plugs,rng:&mut rng});

			let physical_to_logical = physical_to_logical.expect("There were no physical_to_logical");
			let logical_size = logical_size.expect("There were no logical_size");
			let physical_size = physical_size.expect("There were no physical_size");
			let mut map = vec![ vec![];logical_size ];
			for i in 0..logical_size{
				for j in 0..physical_size{
					if physical_to_logical.get_destination(j, &*dummy_topology,&mut rng ) == i
					{
						map[i].push(j);
					}
				}
			}
			map

		});

		let routing=routing.expect("There were no routing");
		ChannelMap{
			routing,
			map,
		}
	}
}





