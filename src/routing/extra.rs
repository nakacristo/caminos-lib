/*!

Extra implementations of routing operations

* Sum (struct SumRouting)
* Stubborn
* EachLengthSourceAdaptiveRouting

*/

use std::cell::RefCell;
use std::convert::TryFrom;

use ::rand::{rngs::StdRng,Rng};

use crate::match_object_panic;
use crate::config_parser::ConfigurationValue;
use crate::routing::*;
use crate::topology::{Topology,Location};

///A policy for the `SumRouting` about how to select among the two `Routing`s.
#[derive(Debug)]
pub enum SumRoutingPolicy
{
	Random,
	TryBoth,
	Stubborn,
	StubbornWhenSecond,
	///Note that both routings are informed of the hops given, which could be illegal for one of them.
	SecondWhenFirstEmpty,
	///At every hop of the first routing give the possibility to use the second routing from the current router towards the target router.
	///once a hop exclussive to the second routing is given continues that way.
	EscapeToSecond,
}

pub fn new_sum_routing_policy(cv: &ConfigurationValue) -> SumRoutingPolicy
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=cv
	{
		match cv_name.as_ref()
		{
			"Random" => SumRoutingPolicy::Random,
			"TryBoth" => SumRoutingPolicy::TryBoth,
			"Stubborn" => SumRoutingPolicy::Stubborn,
			"StubbornWhenSecond" => SumRoutingPolicy::StubbornWhenSecond,
			"SecondWhenFirstEmpty" => SumRoutingPolicy::SecondWhenFirstEmpty,
			"EscapeToSecond" => SumRoutingPolicy::EscapeToSecond,
			_ => panic!("Unknown sum routing policy {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create a SumRoutingPolicy from a non-Object");
	}
}

/// To employ two different routings. It will use either `first_routing` or `second_routing` according to policy.
#[derive(Debug)]
pub struct SumRouting
{
	policy:SumRoutingPolicy,
	//first_routing:Box<dyn Routing>,
	//second_routing:Box<dyn Routing>,
	routing: [Box<dyn Routing>;2],
	//first_allowed_virtual_channels: Vec<usize>,
	//second_allowed_virtual_channels: Vec<usize>,
	allowed_virtual_channels: [Vec<usize>;2],
	//first_extra_label: i32,
	//second_extra_label: i32,
	extra_label: [i32;2],
}

//routin_info.selections uses
//* [a] if a specific routing a has been decided
//* [a,b] if the two routings are available
//* [a,b,c] if a request by routing c has been made, but the two routing are still available.
impl Routing for SumRouting
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, rng: &RefCell<StdRng>) -> RoutingNextCandidates
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
						return RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true};
					}
				}
			}
			unreachable!();
		}
		let meta=routing_info.meta.as_ref().unwrap();
		let r = match routing_info.selections
		{
			None =>
			{
				unreachable!();
			}
			Some(ref s) =>
			{
				//let both = if let &SumRoutingPolicy::TryBoth=&self.policy { routing_info.hops==0 } else { false };
				//if both
				if s.len()>=2
				{
					//let avc0=&self.first_allowed_virtual_channels;
					let avc0=&self.allowed_virtual_channels[0];
					//let el0=self.first_extra_label;
					let el0=self.extra_label[0];
					//let r0=self.first_routing.next(&meta[0].borrow(),topology,current_router,target_server,avc0.len(),rng).into_iter().map( |candidate| CandidateEgress{virtual_channel:avc0[candidate.virtual_channel],label:candidate.label+el0,annotation:Some(RoutingAnnotation{values:vec![0],meta:vec![candidate.annotation]}),..candidate} );
					let r0=self.routing[0].next(&meta[0].borrow(),topology,current_router,target_server,avc0.len(),rng).into_iter().map( |candidate| CandidateEgress{virtual_channel:avc0[candidate.virtual_channel],label:candidate.label+el0,annotation:Some(RoutingAnnotation{values:vec![0],meta:vec![candidate.annotation]}),..candidate} );
					//let avc1=&self.second_allowed_virtual_channels;
					let avc1=&self.allowed_virtual_channels[1];
					//let el1=self.second_extra_label;
					let el1=self.extra_label[1];
					//let r1=self.second_routing.next(&meta[1].borrow(),topology,current_router,target_server,avc1.len(),rng).into_iter().map( |candidate| CandidateEgress{virtual_channel:avc1[candidate.virtual_channel],label:candidate.label+el1,annotation:Some(RoutingAnnotation{values:vec![1],meta:vec![candidate.annotation]}),..candidate} );
					let r1=self.routing[1].next(&meta[1].borrow(),topology,current_router,target_server,avc1.len(),rng).into_iter().map( |candidate| CandidateEgress{virtual_channel:avc1[candidate.virtual_channel],label:candidate.label+el1,annotation:Some(RoutingAnnotation{values:vec![1],meta:vec![candidate.annotation]}),..candidate} );
					match self.policy
					{
						SumRoutingPolicy::SecondWhenFirstEmpty =>
						{
							let r : Vec<_> =r0.collect();
							if r.is_empty() { r1.collect() } else { r }
						}
						_ => r0.chain(r1).collect()
					}
				}
				else
				{
					let index=s[0] as usize;
					//let routing=if s[0]==0 { &self.first_routing } else { &self.second_routing };
					let routing = &self.routing[index];
					//let allowed_virtual_channels=if s[0]==0 { &self.first_allowed_virtual_channels } else { &self.second_allowed_virtual_channels };
					let allowed_virtual_channels = &self.allowed_virtual_channels[index];
					//let extra_label = if s[0]==0 { self.first_extra_label } else { self.second_extra_label };
					let extra_label = self.extra_label[index];
					let r=routing.next(&meta[index].borrow(),topology,current_router,target_server,allowed_virtual_channels.len(),rng);
					//r.into_iter().map( |(x,c)| (x,allowed_virtual_channels[c]) ).collect()
					r.into_iter()
					//.map( |CandidateEgress{port,virtual_channel,label,estimated_remaining_hops}| CandidateEgress{port,virtual_channel:allowed_virtual_channels[virtual_channel],label,estimated_remaining_hops} ).collect()
					.map( |candidate| CandidateEgress{virtual_channel:allowed_virtual_channels[candidate.virtual_channel],label:candidate.label+extra_label,..candidate} ).collect()
				}
			}
		};
		//FIXME: we can recover idempotence in some cases.
		RoutingNextCandidates{candidates:r,idempotent:false}
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_server:usize, rng: &RefCell<StdRng>)
	{
		let all:Vec<i32> = match self.policy
		{
			SumRoutingPolicy::Random => vec![rng.borrow_mut().gen_range(0..2)],
			SumRoutingPolicy::TryBoth | SumRoutingPolicy::Stubborn | SumRoutingPolicy::StubbornWhenSecond
			| SumRoutingPolicy::SecondWhenFirstEmpty | SumRoutingPolicy::EscapeToSecond => vec![0,1],
		};
		let mut bri=routing_info.borrow_mut();
		//bri.meta=Some(vec![RefCell::new(RoutingInfo::new()),RefCell::new(RoutingInfo::new())]);
		bri.meta=Some(vec![RefCell::new(RoutingInfo::new()),RefCell::new(RoutingInfo::new())]);
		for &s in all.iter()
		{
			//let routing=if s==0 { &self.first_routing } else { &self.second_routing };
			let routing = &self.routing[s as usize];
			routing.initialize_routing_info(&bri.meta.as_ref().unwrap()[s as usize],topology,current_router,target_server,rng)
		}
		bri.selections=Some(all);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_server:usize, rng: &RefCell<StdRng>)
	{
		let mut bri=routing_info.borrow_mut();
		let mut cs = match bri.selections
		{
			None => unreachable!(),
			Some(ref t) =>
			{
				if t.len()==3 {
					match self.policy
					{
						SumRoutingPolicy::SecondWhenFirstEmpty => t.clone(),
						_ => vec![t[2]],
						//let s=t[2];
						//bri.selections=Some(vec![s]);
						//s as usize
					}
				} else { t.clone() }

			},
		};
		for &is in cs.iter()
		{
			let s = is as usize;
			let routing = &self.routing[s];
			let meta=bri.meta.as_mut().unwrap();
			meta[s].borrow_mut().hops+=1;
			routing.update_routing_info(&meta[s],topology,current_router,current_port,target_server,rng);
		}
		if let SumRoutingPolicy::EscapeToSecond = self.policy
		{
			if cs[0]==0
			{
				//Readd the escape option
				cs = vec![0,1];
				let second_meta = RefCell::new(RoutingInfo::new());
				self.routing[1].initialize_routing_info(&second_meta,topology,current_router,target_server,rng);
				match bri.meta
				{
					Some(ref mut a) => a[1] = second_meta,
					_ => panic!("No meta data for EscapeToSecond"),
				};
			}
		}
		bri.selections=Some(cs);
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &RefCell<StdRng>)
	{
		//self.first_routing.initialize(topology,rng);
		//self.second_routing.initialize(topology,rng);
		self.routing[0].initialize(topology,rng);
		self.routing[1].initialize(topology,rng);
	}
	fn performed_request(&self, requested:&CandidateEgress, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_server:usize, _num_virtual_channels:usize, _rng:&RefCell<StdRng>)
	{
		let mut bri=routing_info.borrow_mut();
		//if let SumRoutingPolicy::TryBoth=self.policy
		//if let SumRoutingPolicy::Stubborn | SumRoutingPolicy::StubbornWhenSecond =self.policy
		if bri.selections.as_ref().unwrap().len()>1
		{
			let &CandidateEgress{ref annotation,..} = requested;
			if let Some(annotation) = annotation.as_ref()
			{
				let s = annotation.values[0];
				match self.policy
				{
					SumRoutingPolicy::Stubborn => bri.selections=Some(vec![s]),
					SumRoutingPolicy::StubbornWhenSecond => bri.selections = if s==1 {
						Some(vec![1])
					} else {
						Some( vec![ bri.selections.as_ref().unwrap()[0],bri.selections.as_ref().unwrap()[1],s ] )
					},
					_ => bri.selections = Some( vec![ bri.selections.as_ref().unwrap()[0],bri.selections.as_ref().unwrap()[1],s ] ),
				}
			}
		}
		//TODO: recurse over subroutings
	}
	fn statistics(&self, _cycle:usize) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:usize)
	{
	}
}

impl SumRouting
{
	pub fn new(arg: RoutingBuilderArgument) -> SumRouting
	{
		let mut policy=None;
		let mut first_routing=None;
		let mut second_routing=None;
		let mut first_allowed_virtual_channels=None;
		let mut second_allowed_virtual_channels=None;
		let mut first_extra_label=0i32;
		let mut second_extra_label=0i32;
		match_object_panic!(arg.cv,"Sum",value,
			"policy" => policy=Some(new_sum_routing_policy(value)),
			"first_routing" => first_routing=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"second_routing" => second_routing=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"first_allowed_virtual_channels" => first_allowed_virtual_channels = Some(value.as_array()
				.expect("bad value for first_allowed_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in first_allowed_virtual_channels") as usize).collect()),
			"second_allowed_virtual_channels" => second_allowed_virtual_channels = Some(value.as_array()
				.expect("bad value for second_allowed_virtual_channels").iter()
				.map(|v|v.as_f64().expect("bad value in second_allowed_virtual_channels") as usize).collect()),
			"first_extra_label" => first_extra_label = value.as_f64().expect("bad value for first_extra_label") as i32,
			"second_extra_label" => second_extra_label = value.as_f64().expect("bad value for second_extra_label") as i32,
		);
		let policy=policy.expect("There were no policy");
		let first_routing=first_routing.expect("There were no first_routing");
		let second_routing=second_routing.expect("There were no second_routing");
		let first_allowed_virtual_channels=first_allowed_virtual_channels.expect("There were no first_allowed_virtual_channels");
		let second_allowed_virtual_channels=second_allowed_virtual_channels.expect("There were no second_allowed_virtual_channels");
		SumRouting{
			policy,
			//first_routing,
			//second_routing,
			routing: [first_routing,second_routing],
			//first_allowed_virtual_channels,
			//second_allowed_virtual_channels,
			allowed_virtual_channels: [first_allowed_virtual_channels, second_allowed_virtual_channels],
			//first_extra_label,
			//second_extra_label,
			extra_label: [first_extra_label, second_extra_label],
		}
	}
}



///Stubborn routing
///Wraps a routing so that only one request is made in every router.
///The first time the router make a port request, that request is stored and repeated in further calls to `next` until reaching a new router.
///Stores port, virtual_channel, label into routing_info.selections.
///Note that has `idempotent=false` since the value may change if the request has not actually been made.
#[derive(Debug)]
pub struct Stubborn
{
	routing: Box<dyn Routing>,
}

impl Routing for Stubborn
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, rng: &RefCell<StdRng>) -> RoutingNextCandidates
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		if target_router==current_router
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
						return RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true};
					}
				}
			}
			unreachable!();
		}
		if let Some(ref sel)=routing_info.selections
		{
			//return vec![CandidateEgress{port:sel[0] as usize,virtual_channel:sel[1] as usize,label:sel[2],..Default::default()}]
			return RoutingNextCandidates{candidates:vec![CandidateEgress{port:sel[0] as usize,virtual_channel:sel[1] as usize,label:sel[2],..Default::default()}],idempotent:false};
		}
		//return self.routing.next(&routing_info.meta.as_ref().unwrap()[0].borrow(),topology,current_router,target_server,num_virtual_channels,rng)
		//return self.routing.next(&routing_info.meta.as_ref().unwrap()[0].borrow(),topology,current_router,target_server,num_virtual_channels,rng).into_iter().map(|candidate|CandidateEgress{annotation:Some(RoutingAnnotation{values:vec![candidate.label],meta:vec![candidate.annotation]}),..candidate}).collect()
		return RoutingNextCandidates{candidates:self.routing.next(&routing_info.meta.as_ref().unwrap()[0].borrow(),topology,current_router,target_server,num_virtual_channels,rng).into_iter().map(|candidate|CandidateEgress{annotation:Some(RoutingAnnotation{values:vec![candidate.label],meta:vec![candidate.annotation]}),..candidate}).collect(),idempotent:false}
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_server:usize, rng: &RefCell<StdRng>)
	{
		let meta_routing_info=RefCell::new(RoutingInfo::new());
		self.routing.initialize_routing_info(&meta_routing_info, topology, current_router, target_server, rng);
		routing_info.borrow_mut().meta = Some(vec![meta_routing_info]);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_server:usize, rng: &RefCell<StdRng>)
	{
		let mut bri=routing_info.borrow_mut();
		bri.selections=None;
		self.routing.update_routing_info(&bri.meta.as_mut().unwrap()[0],topology,current_router,current_port,target_server,rng);
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &RefCell<StdRng>)
	{
		self.routing.initialize(topology,rng);
	}
	fn performed_request(&self, requested:&CandidateEgress, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_server:usize, num_virtual_channels:usize, rng:&RefCell<StdRng>)
	{
		let &CandidateEgress{port,virtual_channel,ref annotation,..} = requested;
		if let Some(annotation) = annotation.as_ref()
		{
			let label = annotation.values[0];
			//routing_info.borrow_mut().selections=Some(vec![port as i32, virtual_channel as i32, label]);
			let mut bri=routing_info.borrow_mut();
			bri.selections=Some(vec![port as i32, virtual_channel as i32, label]);
			//recurse over routing
			let meta_requested = CandidateEgress{annotation:annotation.meta[0].clone(),..*requested};
			//let meta_info = &routing_info.borrow().meta.as_ref().unwrap()[0];
			let meta_info = &bri.meta.as_ref().unwrap()[0];
			self.routing.performed_request(&meta_requested,meta_info,topology,current_router,target_server,num_virtual_channels,rng);
		}
		//otherwise it is direct to server
	}
	fn statistics(&self, _cycle:usize) -> Option<ConfigurationValue>
	{
		return None;
	}
	fn reset_statistics(&mut self, _next_cycle:usize)
	{
	}
}

impl Stubborn
{
	pub fn new(arg: RoutingBuilderArgument) -> Stubborn
	{
		let mut routing=None;
		match_object_panic!(arg.cv,"Stubborn",value,
			"routing" => routing=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
		);
		let routing=routing.expect("There were no routing");
		Stubborn{
			routing,
		}
	}
}


///Encapsulation of SourceRouting, a variant of SourceAdaptiveRouting. Stores in the packet one path of each length.
///Set label equal to the path length minus the smallest length.
#[derive(Debug)]
pub struct EachLengthSourceAdaptiveRouting
{
	///The base routing
	pub routing: Box<dyn InstantiableSourceRouting>,
}

impl Routing for EachLengthSourceAdaptiveRouting
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
						return RoutingNextCandidates{
							candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),
							idempotent:true
						};
					}
				}
			}
			unreachable!();
		}
		let source_router = routing_info.visited_routers.as_ref().unwrap()[0];
		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);
		let selections = routing_info.selections.as_ref().unwrap().clone();
		for path_index in selections
		{
			let path = &self.routing.get_paths(source_router,target_router)[<usize>::try_from(path_index).unwrap()];
			let next_router = path[routing_info.hops+1];
			let length = path.len() - 1;//substract source router
			let remain = length - routing_info.hops;
			for i in 0..num_ports
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::RouterPort{router_index,router_port:_},_link_class)=topology.neighbour(current_router,i)
				{
					//if distance-1==topology.distance(router_index,target_router)
					if router_index==next_router
					{
						//r.extend((0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)));
						r.extend((0..num_virtual_channels).map(|vc|{
							let mut egress = CandidateEgress::new(i,vc);
							egress.estimated_remaining_hops = Some(remain);
							egress.label = i32::try_from(remain - distance).unwrap();
							egress
						}));
					}
				}
			}
		}
		//println!("From router {} to router {} distance={} cand={}",current_router,target_router,distance,r.len());
		RoutingNextCandidates{candidates:r,idempotent:true}
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_server:usize, rng: &RefCell<StdRng>)
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		routing_info.borrow_mut().visited_routers=Some(vec![current_router]);
		if current_router!=target_router
		{
			let path_collection = self.routing.get_paths(current_router,target_router);
			//println!("path_collection.len={} for source={} target={}\n",path_collection.len(),current_router,target_router);
			if path_collection.is_empty()
			{
				panic!("No path found from router {} to router {}",current_router,target_router);
			}
			let min_length:usize = path_collection.iter().map(|path|path.len()).min().unwrap();
			let max_length:usize = path_collection.iter().map(|path|path.len()).max().unwrap();
			let selected_indices : Vec<i32> = (min_length..=max_length).filter_map(|length|{
				//get some random path with the given length
				let candidates : Vec<usize> = (0..path_collection.len()).filter(|&index|path_collection[index].len()==length).collect();
				if candidates.is_empty() {
					None
				} else {
					let r = rng.borrow_mut().gen_range(0..candidates.len());
					Some(i32::try_from(candidates[r]).unwrap())
				}
			}).collect();
			routing_info.borrow_mut().selections=Some(selected_indices);
		}
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, _current_port:usize, target_server:usize, _rng: &RefCell<StdRng>)
	{
		let (target_location,_link_class)=topology.server_neighbour(target_server);
		let target_router=match target_location
		{
			Location::RouterPort{router_index,router_port:_} =>router_index,
			_ => panic!("The server is not attached to a router"),
		};
		let mut ri=routing_info.borrow_mut();
		let hops = ri.hops;
		if let Some(ref mut visited)=ri.visited_routers
		{
			let source_router = visited[0];
			visited.push(current_router);
			//Now discard all selections toward other routers.
			let paths = &self.routing.get_paths(source_router,target_router);
			if let Some(ref mut selections)=ri.selections
			{
				selections.retain(|path_index|{
					let path = &paths[<usize>::try_from(*path_index).unwrap()];
					path[hops]==current_router
				});
				if selections.is_empty()
				{
					panic!("No selections remaining.");
				}
			}
		}
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &RefCell<StdRng>)
	{
		self.routing.initialize(topology,rng);
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

