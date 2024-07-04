/*!

Extra implementations of routing operations

* Sum (struct SumRouting)
* Stubborn
* EachLengthSourceAdaptiveRouting

*/

use std::cell::RefCell;
use std::collections::HashSet;
use std::convert::TryFrom;
use std::ops::Deref;

use ::rand::{rngs::StdRng,Rng};
use rand::SeedableRng;

use crate::match_object_panic;
use crate::config_parser::ConfigurationValue;
use crate::matrix::Matrix;
use crate::pattern::{new_pattern, Pattern, PatternBuilderArgument};
use crate::routing::*;
use crate::topology::prelude::*;
//use crate::topology::{Topology,Location};

///A policy for the `SumRouting` about how to select among the two `Routing`s.
#[derive(Debug)]
pub enum SumRoutingPolicy
{
	///Random at source.
	Random,
	///Keep both options as long as possible.
	TryBoth,
	///Keep both options as long as possible. Preserve made decisions.
	Stubborn,
	StubbornWhenSecond,
	///Note that both routings are informed of the hops given, which could be illegal for one of them.
	SecondWhenFirstEmpty,
	///At every hop of the first routing give the possibility to use the second routing from the current router towards the target router.
	///once a hop exclusive to the second routing is given continues that way.
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
	//
	enabled_statistics: bool,
	//when capturing statistics track the hops of each kind.
	tracked_hops: RefCell<[i64;2]>,
}

//routin_info.selections uses
//* [a] if a specific routing a has been decided
//* [a,b] if the two routings are available
//* [a,b,c] if a request by routing c has been made, but the two routing are still available.
impl Routing for SumRouting
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
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
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true});
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
					let r0=self.routing[0].next(&meta[0].borrow(),topology,current_router,target_router,target_server,avc0.len(),rng)?.into_iter().map( |candidate| CandidateEgress{virtual_channel:avc0[candidate.virtual_channel],label:candidate.label+el0,annotation:Some(RoutingAnnotation{values:vec![0],meta:vec![candidate.annotation]}),..candidate} );
					//let avc1=&self.second_allowed_virtual_channels;
					let avc1=&self.allowed_virtual_channels[1];
					//let el1=self.second_extra_label;
					let el1=self.extra_label[1];
					//let r1=self.second_routing.next(&meta[1].borrow(),topology,current_router,target_server,avc1.len(),rng).into_iter().map( |candidate| CandidateEgress{virtual_channel:avc1[candidate.virtual_channel],label:candidate.label+el1,annotation:Some(RoutingAnnotation{values:vec![1],meta:vec![candidate.annotation]}),..candidate} );
					let r1=self.routing[1].next(&meta[1].borrow(),topology,current_router,target_router,target_server,avc1.len(),rng)?.into_iter().map( |candidate| CandidateEgress{virtual_channel:avc1[candidate.virtual_channel],label:candidate.label+el1,annotation:Some(RoutingAnnotation{values:vec![1],meta:vec![candidate.annotation]}),..candidate} );
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
					let r=routing.next(&meta[index].borrow(),topology,current_router,target_router,target_server,allowed_virtual_channels.len(),rng)?;
					//r.into_iter().map( |(x,c)| (x,allowed_virtual_channels[c]) ).collect()
					r.into_iter()
					//.map( |candidate| CandidateEgress{virtual_channel:allowed_virtual_channels[candidate.virtual_channel],label:candidate.label+extra_label,..candidate} ).collect()
					// We need to keep the annotation to have a coherent state able to relay the annotation of the subrouting.
					.map( |candidate| CandidateEgress{virtual_channel:allowed_virtual_channels[candidate.virtual_channel],label:candidate.label+extra_label,annotation:Some(RoutingAnnotation{values:vec![s[0]],meta:vec![candidate.annotation]}),..candidate} ).collect()
				}
			}
		};
		//FIXME: we can recover idempotence in some cases.
		Ok(RoutingNextCandidates{candidates:r,idempotent:false})
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		let all:Vec<i32> = match self.policy
		{
			SumRoutingPolicy::Random => vec![rng.gen_range(0..2)],
			SumRoutingPolicy::TryBoth | SumRoutingPolicy::Stubborn | SumRoutingPolicy::StubbornWhenSecond
			| SumRoutingPolicy::SecondWhenFirstEmpty | SumRoutingPolicy::EscapeToSecond => vec![0,1],
		};
		let mut bri=routing_info.borrow_mut();

		let mut routing_info_1 = RoutingInfo::new();
		routing_info_1.source_server = Some(bri.source_server.unwrap());
		let mut routing_info_2 = RoutingInfo::new();
		routing_info_2.source_server = Some(bri.source_server.unwrap());
		bri.meta=Some(vec![RefCell::new(routing_info_1),RefCell::new(routing_info_2)]);

		for &s in all.iter()
		{
			//let routing=if s==0 { &self.first_routing } else { &self.second_routing };
			let routing = &self.routing[s as usize];
			routing.initialize_routing_info(&bri.meta.as_ref().unwrap()[s as usize],topology,current_router,target_router,target_server,rng)
		}
		bri.selections=Some(all);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		use SumRoutingPolicy::*;
		let mut bri=routing_info.borrow_mut();
		if self.enabled_statistics
		{
			if let Some(cs) = &bri.selections
			{
				let tracked_hops = &mut self.tracked_hops.borrow_mut();
				let range = if cs.len()==3 { &cs[2..=2] } else { &cs[..] };
				//let range = match &self.policy
				//{
				//	SecondWhenFirstEmpty => &cs[2..=2],
				//	_ =>
				//	{
				//		let limit = cs.len().min(2);
				//		&cs[0..limit]
				//	}
				//};
				for &is in range.iter()
				{
					tracked_hops[is as usize] +=1;
				}
			}
		}
		let mut cs = match bri.selections
		{
			None => unreachable!(),
			Some(ref t) =>
			{
				if t.len()==3 {
					match self.policy
					{
						SecondWhenFirstEmpty => t.clone(),
						_ => vec![t[2]],
						//let s=t[2];
						//bri.selections=Some(vec![s]);
						//s as usize
					}
				} else { t.clone() }
			},
		};
		for &is in cs.iter().take(2)
		{
			let s = is as usize;
			let routing = &self.routing[s];
			let meta=bri.meta.as_mut().unwrap();
			meta[s].borrow_mut().hops+=1;
			routing.update_routing_info(&meta[s],topology,current_router,current_port,target_router,target_server,rng);
		}
		if let EscapeToSecond = self.policy
		{
			if cs[0]==0
			{
				//Read the escape option
				cs = vec![0,1];
				let second_meta = RefCell::new(RoutingInfo::new());
				self.routing[1].initialize_routing_info(&second_meta,topology,current_router,target_router,target_server,rng);
				match bri.meta
				{
					Some(ref mut a) => a[1] = second_meta,
					_ => panic!("No meta data for EscapeToSecond"),
				};
			}
		}
		bri.selections=Some(cs);
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng)
	{
		//self.first_routing.initialize(topology,rng);
		//self.second_routing.initialize(topology,rng);
		self.routing[0].initialize(topology,rng);
		self.routing[1].initialize(topology,rng);
	}
	fn performed_request(&self, requested:&CandidateEgress, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, _num_virtual_channels:usize, rng:&mut StdRng)
	{
		use sum_routing_internal::{SumRoutingSelection,SumRoutingCase::*};
		let mut bri=routing_info.borrow_mut();
		//if bri.selections.as_ref().unwrap().len()>1
		if let DoubleChoice(..) = bri.selections.case()
		{
			let &CandidateEgress{ref annotation,..} = requested;
			if let Some(annotation) = annotation.as_ref()
			{
				let s = annotation.values[0];
				match self.policy
				{
					//SumRoutingPolicy::Stubborn => bri.selections=Some(vec![s]),
					SumRoutingPolicy::Stubborn => bri.selections.set_single(s),
					//SumRoutingPolicy::StubbornWhenSecond => bri.selections = if s==1 {
					//	Some(vec![1])
					//} else {
					//	Some( vec![ bri.selections.as_ref().unwrap()[0],bri.selections.as_ref().unwrap()[1],s ] )
					//},
					SumRoutingPolicy::StubbornWhenSecond => if s==1 {
						bri.selections.set_single(1);
					} else {
						bri.selections.set_request(s);
					},
					//_ => bri.selections = Some( vec![ bri.selections.as_ref().unwrap()[0],bri.selections.as_ref().unwrap()[1],s ] ),
					_ => bri.selections.set_request(s),
				}
			}
		}
		let &CandidateEgress{ref annotation,..} = requested;
		if let Some(annotation) = annotation.as_ref()
		{
			let meta=bri.meta.as_mut().unwrap();
			let mut sub_requested = requested.clone();
			let s = annotation.values[0] as usize;
			let routing = &self.routing[s];
			sub_requested.annotation = requested.annotation.as_ref().unwrap().meta[0].clone();
			let sub_num_vc = self.allowed_virtual_channels[s].len();
			routing.performed_request(&sub_requested,&meta[s],topology,current_router,target_router,target_server,sub_num_vc,rng);
		}
	}
	fn statistics(&self, cycle:Time) -> Option<ConfigurationValue>
	{
		if self.enabled_statistics {
			let tracked_hops = self.tracked_hops.borrow();
			let mut content = vec![
				(String::from("first_routing_hops"),ConfigurationValue::Number(tracked_hops[0] as f64)),
				(String::from("second_routing_hops"),ConfigurationValue::Number(tracked_hops[1] as f64)),
			];
			if let Some(inner)=self.routing[0].statistics(cycle)
			{
				content.push( (String::from("first_statistics"),inner) );
			}
			if let Some(inner)=self.routing[1].statistics(cycle)
			{
				content.push( (String::from("second_statistics"),inner) );
			}
			Some(ConfigurationValue::Object(String::from("SumRoutingStatistics"),content))
		} else {
			None
		}
	}
	fn reset_statistics(&mut self, _next_cycle:Time)
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
		let mut enabled_statistics=false;
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
			"enabled_statistics" => enabled_statistics = value.as_bool().expect("bad value for enabled_statistics"),
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
			enabled_statistics,
			tracked_hops: RefCell::new([0,0]),
		}
	}
}

mod sum_routing_internal
{
	pub trait SumRoutingSelection
	{
		fn case(&self) -> SumRoutingCase;
		/// Set a single routing as selected.
		fn set_single(&mut self, selection:i32);
		/// Mark a request as been performed.
		fn set_request(&mut self, request:i32);
	}
	use SumRoutingCase::*;
	impl SumRoutingSelection for Option<Vec<i32>>
	{
		fn case(&self) -> SumRoutingCase
		{
			if let Some(s) = self {
				if s.len()==1 {
					SingleChoice(s[0])
				} else {
					DoubleChoice(s[0],s[1])
				}
			} else {
				panic!("Invalid selections");
			}
		}
		fn set_single(&mut self, selection:i32)
		{
			*self = Some(vec![selection]);
		}
		fn set_request(&mut self, request:i32)
		{
			if let Some(ref mut s) = self {
				if s.len()>=2 {
					*s = vec![s[0],s[1],request];
				}
			}
		}
	}
	#[allow(dead_code)]
	pub enum SumRoutingCase
	{
		SingleChoice(i32),
		DoubleChoice(i32,i32),
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
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		//let (target_location,_link_class)=topology.server_neighbour(target_server);
		//let target_router=match target_location
		//{
		//	Location::RouterPort{router_index,router_port:_} =>router_index,
		//	_ => panic!("The server is not attached to a router"),
		//};
		if target_router==current_router
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
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true});
					}
				}
			}
			unreachable!();
		}
		if let Some(ref sel)=routing_info.selections
		{
			//return vec![CandidateEgress{port:sel[0] as usize,virtual_channel:sel[1] as usize,label:sel[2],..Default::default()}]
			return Ok(RoutingNextCandidates{candidates:vec![CandidateEgress{port:sel[0] as usize,virtual_channel:sel[1] as usize,label:sel[2],..Default::default()}],idempotent:false});
		}
		//return self.routing.next(&routing_info.meta.as_ref().unwrap()[0].borrow(),topology,current_router,target_server,num_virtual_channels,rng)
		//return self.routing.next(&routing_info.meta.as_ref().unwrap()[0].borrow(),topology,current_router,target_server,num_virtual_channels,rng).into_iter().map(|candidate|CandidateEgress{annotation:Some(RoutingAnnotation{values:vec![candidate.label],meta:vec![candidate.annotation]}),..candidate}).collect()
		return Ok(RoutingNextCandidates{candidates:self.routing.next(&routing_info.meta.as_ref().unwrap()[0].borrow(),topology,current_router,target_router,target_server,num_virtual_channels,rng)?.into_iter().map(|candidate|CandidateEgress{annotation:Some(RoutingAnnotation{values:vec![candidate.label],meta:vec![candidate.annotation]}),..candidate}).collect(),idempotent:false})
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		let meta_routing_info=RefCell::new(RoutingInfo::new());
		self.routing.initialize_routing_info(&meta_routing_info, topology, current_router, target_router, target_server, rng);
		routing_info.borrow_mut().meta = Some(vec![meta_routing_info]);
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, current_port:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		let mut bri=routing_info.borrow_mut();
		bri.selections=None;
		self.routing.update_routing_info(&bri.meta.as_mut().unwrap()[0],topology,current_router,current_port,target_router,target_server,rng);
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.routing.initialize(topology,rng);
	}
	fn performed_request(&self, requested:&CandidateEgress, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, num_virtual_channels:usize, rng:&mut StdRng)
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
			self.routing.performed_request(&meta_requested,meta_info,topology,current_router,target_router,target_server,num_virtual_channels,rng);
		}
		//otherwise it is direct to server
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
						return Ok(RoutingNextCandidates{
							candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),
							idempotent:true
						});
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
		Ok(RoutingNextCandidates{candidates:r,idempotent:true})
	}
	fn initialize_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, current_router:usize, target_router:usize, _target_server:Option<usize>, rng: &mut StdRng)
	{
		//let (target_location,_link_class)=topology.server_neighbour(target_server);
		//let target_router=match target_location
		//{
		//	Location::RouterPort{router_index,router_port:_} =>router_index,
		//	_ => panic!("The server is not attached to a router"),
		//};
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
					let r = rng.gen_range(0..candidates.len());
					Some(i32::try_from(candidates[r]).unwrap())
				}
			}).collect();
			routing_info.borrow_mut().selections=Some(selected_indices);
		}
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, current_router:usize, _current_port:usize, target_router:usize, _target_server:Option<usize>, _rng: &mut StdRng)
	{
		//let (target_location,_link_class)=topology.server_neighbour(target_server);
		//let target_router=match target_location
		//{
		//	Location::RouterPort{router_index,router_port:_} =>router_index,
		//	_ => panic!("The server is not attached to a router"),
		//};
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
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.routing.initialize(topology,rng);
	}
}


/**
Begins including all neighbours until some condition. Then use an underlying routing until the destination.

See for example Adaptive Clos (CLOS AD, from "Flattened Butterfly : A Cost-Efficient Topology for High-Radix Networks"
by John Kim, William J. Dally, and Dennis Abts. ISCA'27.), where a packet is routing in the Hamming topology adaptatively alike in a Clos Network.
Going though the queues with least occupation until reaching a root. To emulate its initial DOR requirement ...TODO...

```ignore
AdaptiveStart{
	adaptive_hops: 3,
	//adaptive_label: 0,
	routing: Shortest,
}
```

**/
#[derive(Debug)]
pub struct AdaptiveStart
{
	adaptive_hops: usize,
	routing: Box<dyn Routing>,
	adaptive_label: i32,
}

impl Routing for AdaptiveStart
{
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router: usize, target_server:Option<usize>, num_virtual_channels:usize, rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>
	{
		if target_router==current_router
		{
			let target_server = target_server.expect("target server was not given.");
			for i in 0..topology.ports(current_router)
			{
				//println!("{} -> {:?}",i,topology.neighbour(current_router,i));
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router,i)
				{
					if server==target_server
					{
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
					}
				}
			}
			unreachable!();
		}
		if let Some(ref meta) = routing_info.meta {
			assert_eq!(meta.len(),1);
			return self.routing.next(&meta[0].borrow(),topology,current_router,target_router,target_server,num_virtual_channels,rng);
		}
		let mut r =Vec::with_capacity(topology.ports(current_router)*num_virtual_channels);
		for NeighbourRouterIteratorItem{port_index,..} in topology.neighbour_router_iter(current_router)
		{
			r.extend((0..num_virtual_channels).map(|vc|{
				let mut egress = CandidateEgress::new(port_index,vc);
				egress.label = self.adaptive_label;
				egress
			}));
		}
		Ok(RoutingNextCandidates{candidates:r,idempotent:true})
	}
	fn update_routing_info(&self, routing_info:&RefCell<RoutingInfo>, topology:&dyn Topology, current_router:usize, _current_port:usize, target_router:usize, target_server:Option<usize>, rng: &mut StdRng)
	{
		let mut bri = routing_info.borrow_mut();
		if let None = bri.meta {
			if bri.hops >= self.adaptive_hops {
				let info = RefCell::new(RoutingInfo::new());
				self.routing.initialize_routing_info(&info,topology,current_router,target_router,target_server,rng);
				bri.meta = Some(vec![info]);
			}
		}
	}
}

impl AdaptiveStart
{
	pub fn new(arg: RoutingBuilderArgument) -> AdaptiveStart
	{
		let mut adaptive_hops = None;
		let mut routing = None;
		let mut adaptive_label = 0i32;
		match_object_panic!(arg.cv,"AdaptiveStart",value,
			"adaptive_hops" => adaptive_hops = Some(value.as_usize().expect("bad value for adaptive_hops")),
			"routing" => routing=Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"adaptive_label" => adaptive_label = value.as_i32().expect("bad value for adaptive_label"),
		);
		let adaptive_hops = adaptive_hops.expect("missing adaptive_hops");
		if adaptive_hops == 0 {
			panic!("adaptive_hops cannot be 0.");
		}
		let routing = routing.expect("missing routing");
		AdaptiveStart{
			adaptive_hops,
			routing,
			adaptive_label,
		}
	}
}

/**
Routing that embeds a logical topology and a logical routing over the physical topology.
Each router is mapped to a router in the logical topology.
All logical connections are mapped to physical connections, and the remaining physical connections are used to opportunistically route.
An opportunistic hop can be made if the hop nears the logical target router in the logical topology.
# Example
```ignore
SubTopologyRouting{
	logical_topology: Hamming{ //Hypercube
		servers_per_router: 2, //useless
		sides:[2,2],
	},
	map:Identity,
	logical_routing: DOR{order:[0,1]},
	opportunistic_hops:true,
	opportunistic_set_label:0,
	legend_name: "Hypercube-DOR opportunistic"
}
```
**/

#[derive(Debug)]
pub struct SubTopologyRouting
{
	logical_topology: Box<dyn Topology>,
	map: Box<dyn Pattern>,
	physical_to_logical: Vec<usize>,
	logical_to_physical: Vec<usize>,
	logical_topology_connections: Matrix<usize>,
	logical_routing: Box<dyn Routing>,
	opportunistic_hops: bool,
}

impl Routing for SubTopologyRouting
{
	fn next(&self, routing_info: &RoutingInfo, topology: &dyn Topology, current_router: usize, target_router: usize, target_server: Option<usize>, num_virtual_channels: usize, rng: &mut StdRng) -> Result<RoutingNextCandidates, Error> {
		if target_router == current_router
		{
			let target_server = target_server.expect("target server was not given.");
			for i in 0..topology.ports(current_router)
			{
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router, i)
				{
					if server==target_server
					{
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
					}
				}
			}
			unreachable!();
		}

		let logical_current = self.physical_to_logical[current_router];
		let logical_target  = self.physical_to_logical[ target_router];
		let logical_candidates = self.logical_routing.next(&routing_info.meta.as_ref().unwrap()[0].borrow(), self.logical_topology.as_ref(), logical_current, logical_target, None, num_virtual_channels, rng)?;
		let mut candidates =vec![];
		for CandidateEgress{port,virtual_channel,label: _,annotation,..} in logical_candidates.candidates
		{
			let Location::RouterPort{router_index: next_physical_router, .. } = self.logical_topology.neighbour(logical_current, port).0 else { panic!("There should be a port") };
			let physical_port = topology.neighbour_router_iter(current_router).find(|item| item.neighbour_router == next_physical_router).expect("port not found").port_index;
			//let physical_label = label;

			let label = if topology.distance(next_physical_router, target_router) < topology.distance(current_router, target_router)
			{
				0

			} else if topology.distance(next_physical_router, target_router) == topology.distance(current_router, target_router)
			{
				1

			}else {

				2

			};
			candidates.push(CandidateEgress{port:physical_port,virtual_channel,label, estimated_remaining_hops: None, router_allows: None, annotation});
		}

		if self.opportunistic_hops
		{
			for neighbour in topology.neighbour_router_iter(current_router).into_iter()
			{
				let physical_neighbour = neighbour.neighbour_router;
				let logical_neighbour = self.physical_to_logical[physical_neighbour];
				if self.logical_topology.distance(logical_neighbour, logical_target) < self.logical_topology.distance(logical_current, logical_target)
					&& *self.logical_topology_connections.get(current_router, physical_neighbour) == 0
				{
					let physical_port = neighbour.port_index;
					let label = if topology.distance(physical_neighbour, target_router) < topology.distance(current_router, target_router)
					{
						0

					} else if topology.distance(physical_neighbour, target_router) == topology.distance(current_router, target_router)
					{
						1

					}else {

						2

					};
					candidates.extend((0..num_virtual_channels).map(|vc|CandidateEgress{port:physical_port,virtual_channel:vc,label, estimated_remaining_hops: None, router_allows: None, annotation: None}));
				}

			}
		}
		Ok(RoutingNextCandidates{candidates, idempotent: logical_candidates.idempotent})
	}

	fn initialize_routing_info(&self, routing_info: &RefCell<RoutingInfo>, _topology: &dyn Topology, current_router: usize, target_router: usize, _target_server: Option<usize>, rng: &mut StdRng) {
		let logical_current = self.physical_to_logical[current_router];
		let logical_target = self.physical_to_logical[target_router];

		let mut bri = routing_info.borrow_mut();
		bri.meta = Some(vec![ RefCell::new(RoutingInfo::new()) ]);
		let bri_sub = &bri.meta.as_ref().unwrap()[0];
		self.logical_routing.initialize_routing_info(bri_sub, self.logical_topology.as_ref(), logical_current, logical_target, None, rng);
	}

	fn update_routing_info(&self, routing_info: &RefCell<RoutingInfo>, topology: &dyn Topology, current_router: usize, current_port: usize, target_router: usize, _target_server: Option<usize>, rng: &mut StdRng) {
		let logical_current = self.physical_to_logical[current_router];
		let logical_target = self.physical_to_logical[target_router];
		let (previous_physical_router_loc, _link_class) = topology.neighbour(current_router, current_port);

		// println!("current_router={}, current_port={}, previous_physical_router_loc={:?}", current_router, current_port, previous_physical_router_loc);
		//TODO: Reduce the complexity of this operation. It can be O(1) instead of O(degree) but a new method is needed in the trait.
		let mut routing_info = routing_info.borrow_mut();
		if let Location::RouterPort{router_index: previous_physical_router,..} = previous_physical_router_loc {
			let prev_logical_router = self.physical_to_logical[previous_physical_router];
			if let Some(a) = self.logical_topology.neighbour_router_iter(logical_current)
				.find(|item| item.neighbour_router == prev_logical_router)
			{
				let logical_port  = a.port_index;
				self.logical_routing.update_routing_info(&(routing_info.meta.as_ref().unwrap()[0]), self.logical_topology.as_ref(), logical_current, logical_port, logical_target, None, rng);
			}else{
				let routing_info_sub = RefCell::new(RoutingInfo::new());
				routing_info.meta = Some(vec![routing_info_sub]);

				self.logical_routing.initialize_routing_info(&routing_info.meta.as_ref().unwrap()[0], self.logical_topology.as_ref(), logical_current, logical_target, None, rng);
			}
		}else {
			panic!("!!")
		}
	}

	fn initialize(&mut self, topology: &dyn Topology, rng: &mut StdRng) {

		self.map.initialize(self.logical_topology.num_routers(), self.logical_topology.num_routers(), self.logical_topology.as_ref(), rng);
		for i in 0..self.logical_topology.num_routers() {
			let physical = self.map.get_destination(i, topology, rng);
			self.logical_to_physical[i] = physical;
			self.physical_to_logical[physical] = i;
		}

		//Check that neighbours in the logical topology are neighbours in the physical topology
		self.logical_topology_connections = Matrix::constant(0,topology.num_routers(), topology.num_routers()); //physical matrix but logical connections
		//TODO: Reduce the complexity of this operation.
		for i in 0..self.logical_topology.num_routers() {
			let physical_i = self.logical_to_physical[i];
			for NeighbourRouterIteratorItem{neighbour_router,..} in self.logical_topology.neighbour_router_iter(i) {
				let physical_neighbour = self.logical_to_physical[neighbour_router];
				let neighbour = topology.neighbour_router_iter(physical_i).find(|item| item.neighbour_router == physical_neighbour).is_some();
				assert!(neighbour);
				*self.logical_topology_connections.get_mut(physical_i, physical_neighbour) = 1;

			}
		}
		// println!("logical_topology_connections={:?}",self.logical_topology_connections);

		self.logical_routing.initialize(self.logical_topology.as_ref(), rng);
	}

	fn statistics(&self, _cycle: Time) -> Option<ConfigurationValue> {
		None
	}
}

impl SubTopologyRouting
{
	pub fn new(arg: RoutingBuilderArgument) -> SubTopologyRouting
	{
		let mut logical_topology = None;
		let mut map = None;
		let mut logical_routing = None;
		let mut opportunistic_hops = false;
		//new rng for the subtopology
		let rng =  &mut StdRng::from_entropy();
		match_object_panic!(arg.cv,"SubTopologyRouting",value,
			"logical_topology" => logical_topology = Some(new_topology(TopologyBuilderArgument{cv:value, rng, plugs: arg.plugs})),
			"map" => map = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})), //map of the application over the machine
			"logical_routing" => logical_routing = Some(new_routing(RoutingBuilderArgument{cv:value,..arg})),
			"opportunistic_hops" => opportunistic_hops = value.as_bool().expect("bad value for opportunistic_hops"),
		);
		let logical_topology = logical_topology.expect("missing topology");
		let map = map.expect("missing physical_to_logical");
		let logical_routing = logical_routing.expect("missing routing");

		let physical_to_logical = vec![0; logical_topology.num_routers()];
		let logical_to_physical = vec![0; logical_topology.num_routers()];


		SubTopologyRouting {
			logical_topology,
			map,
			physical_to_logical,
			logical_to_physical,
			logical_topology_connections: Matrix::constant(0,0,0),
			logical_routing,
			opportunistic_hops,
		}
	}
}

/**
Routing that selects a routing based on the current and next candiate routers.
If the pair of routers belongs to the same region, the routing is selected from the region routing. If not, the default routing is selected.
Main use is to mark faulty regions and use a different routing for them.
# Example
```ignore
RegionRouting{
	physical_to_logical:[
		LinearTransform{
			source_size:[4,4],
			matrix:[[1,0]],
			target_size:[4],
		},
	],
	selected_region_size:[4],
	logical_to_physical:[
		LinearTransform{
			source_size:[4],
			matrix:[[1], [0]],
			target_size:[4,4],
		},

	],
	region_logical_topology:[
		Hamming{
			sides:[4],
			servers_per_router:4,
		},
	],
	routings:[
		SubTopologyRouting{
			logical_topology: Hamming //Hypercube
			{
				servers_per_router: 2, //useless
				sides:[2,2],
			},
			map:Identity,
			logical_routing: DOR{order:[0,1]},
			opportunistic_hops:true,
			opportunistic_set_label:0,
			legend_name: "Hypercube-DOR opportunistic"
		},

	],
	extra_label_selection: 4,
	default_routing: Polarized{include_labels:true,strong:false, panic_on_empty:false},
	legend_name: "Fault tolerant routing",
},

**/
#[derive(Debug)]
pub struct RegionRouting
{
	physical_to_logical: Vec<Box<dyn Pattern>>,
	logical_to_physical: Vec<Box<dyn Pattern>>,
	selected_region_size: Vec<usize>,
	physical_to_logical_vector: Vec<Vec<usize>>,
	logical_to_physical_vector: Vec<Vec<usize>>,
	region_logical_topology: Vec<Box<dyn Topology>>,
	routings: Vec<Box<dyn Routing>>,
	extra_label_selection: i32,
	default_routing: Box<dyn Routing>,
}

impl Routing for RegionRouting
{
	fn next(&self, routing_info: &RoutingInfo, topology: &dyn Topology, current_router: usize, target_router: usize, target_server: Option<usize>, num_virtual_channels: usize, rng: &mut StdRng) -> Result<RoutingNextCandidates, Error> {

		if current_router == target_router
		{
			let target_server = target_server.expect("target server was not given.");
			for i in 0..topology.ports(current_router)
			{
				if let (Location::ServerPort(server),_link_class)=topology.neighbour(current_router, i)
				{
					if server==target_server
					{
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true})
					}
				}
			}
			unreachable!();
		}

		let default_bri = routing_info.meta.as_ref().unwrap()[0].borrow();
		let next = self.default_routing.next(default_bri.deref(), topology, current_router, target_router, target_server, num_virtual_channels, rng)?;
		let mut candidates = vec![];
		let mut selections = HashSet::new();
		'outer: for CandidateEgress{port, virtual_channel, label, annotation, router_allows, estimated_remaining_hops} in next.candidates
		{
			let Location::RouterPort {router_index: next_router, router_port:_} = topology.neighbour(current_router, port).0 else { panic!("There should be a port")};
			for (i, (ptlv, ltpv)) in self.physical_to_logical_vector.iter().zip(self.logical_to_physical_vector.iter()).enumerate()
			{
				if ltpv[ptlv[current_router]] == current_router && ltpv[ptlv[next_router]] == next_router
				{
					selections.insert(i);
					continue 'outer;
				}
			}

			candidates.push(CandidateEgress{port, virtual_channel, label, annotation, router_allows, estimated_remaining_hops});

		}
		for i in selections
		{
			let selected_bri = &routing_info.meta.as_ref().unwrap()[i +1];
			let current_logical = self.physical_to_logical_vector[i][current_router];
			let target_logical = self.physical_to_logical_vector[i][target_router];
			if current_logical != target_logical
			{
				let next = self.routings[i].next(selected_bri.borrow().deref(), self.region_logical_topology[i].as_ref(), current_logical, target_logical, None, num_virtual_channels, rng)?;
				for CandidateEgress{port, virtual_channel, label, annotation, router_allows, estimated_remaining_hops} in next.candidates
				{
					let Location::RouterPort {router_index: next_router, router_port:_port_logical} = self.region_logical_topology[i].neighbour(current_logical, port).0 else { panic!("There should be a port")};
					let next_physical = self.logical_to_physical_vector[i][next_router];
					let physical_port = topology.neighbour_router_iter(current_router).find(|item| item.neighbour_router == next_physical).expect("port not found").port_index;
					candidates.push(CandidateEgress{port:physical_port, virtual_channel, label: label + self.extra_label_selection, annotation, router_allows, estimated_remaining_hops});
				}
			}
		}

		Ok(RoutingNextCandidates{candidates, idempotent: false})
	}

	fn initialize_routing_info(&self, routing_info: &RefCell<RoutingInfo>, topology: &dyn Topology, current_router: usize, target_router: usize, target_server: Option<usize>, rng: &mut StdRng) {
		let routing_info_default = RefCell::new(RoutingInfo::new());
		let mut all_routing_info = vec![routing_info_default];
		self.default_routing.initialize_routing_info(&all_routing_info[0], topology, current_router, target_router, target_server, rng);
		//initialize the routing info for each routing

		for (i,routing) in self.routings.iter().enumerate() {
			let routing_info = RefCell::new(RoutingInfo::new());
			let current_logical = self.physical_to_logical_vector[i][current_router];
			let target_logical = self.physical_to_logical_vector[i][target_router];
			routing.initialize_routing_info(&routing_info, self.region_logical_topology[i].as_ref(), current_logical, target_logical, target_server, rng);
			all_routing_info.push(routing_info);
		}
		routing_info.borrow_mut().meta = Some(all_routing_info);
	}

	fn update_routing_info(&self, routing_info: &RefCell<RoutingInfo>, topology: &dyn Topology, current_router: usize, current_port: usize, target_router: usize, target_server: Option<usize>, rng: &mut StdRng) {
		let Location::RouterPort {router_index: previous_router, router_port:_} = topology.neighbour(current_router, current_port).0 else { panic!("There should be a port")};
		let mut pattern = None;

		for (i, (ptlv, ltpv)) in self.physical_to_logical_vector.iter().zip(self.logical_to_physical_vector.iter()).enumerate()
		{
			if ltpv[ptlv[current_router]] == current_router && ltpv[ptlv[previous_router]] == previous_router
			{
				pattern = Some(i);
				break;
			}
		}

		let bri = routing_info.borrow();

		for i in 0..self.routings.len() //IMPORTANT TO UPDATE ALL ROUTINGS not used
		{
			if !pattern.is_some() || pattern.unwrap() != i
			{
				bri.meta.as_ref().unwrap()[i +1].replace(RoutingInfo::new());
				let routing_bri = &(bri.meta.as_ref().unwrap()[i + 1]);
				let current_logical = self.physical_to_logical_vector[i][current_router];
				let target_logical = self.physical_to_logical_vector[i][target_router];
				self.routings[i].initialize_routing_info(routing_bri, self.region_logical_topology[i].as_ref(), current_logical, target_logical, target_server, rng);
			}
		}

		if let Some(pattern) = pattern
		{
			let current_logical = self.physical_to_logical_vector[pattern][current_router];
			let target_logical = self.physical_to_logical_vector[pattern][target_router];
			//get previous physical router with the port
			let Location::RouterPort {router_index: previous_physical_router, router_port:_} = topology.neighbour(current_router, current_port).0 else { panic!("There should be a port")};
			let previous_logical_router = self.physical_to_logical_vector[pattern][previous_physical_router];
			//now get the logical port iterating the logical neighbours
			let logical_port = self.region_logical_topology[pattern].neighbour_router_iter(current_logical).find(|item| item.neighbour_router == previous_logical_router).expect("port not found").port_index;

			let routing_bri = &(bri.meta.as_ref().unwrap()[pattern + 1]);
			self.routings[pattern].update_routing_info(routing_bri, self.region_logical_topology[pattern].as_ref(), current_logical, logical_port, target_logical, target_server, rng);
			// if  current_router != target_router {
			// 	//if the next in the default routing is not giving more candidates for this pattern in the current router, update the default routing
			// 	let bri_default = bri.meta.as_ref().unwrap()[0].borrow();
			// 	let next = self.default_routing.next(bri_default.deref(), topology, current_router, target_router, target_server, 1, rng).unwrap();
			// 	for CandidateEgress { port, .. } in next.candidates
			// 	{
			// 		let Location::RouterPort { router_index: next_router, router_port: _ } = topology.neighbour(current_router, port).0 else { panic!("There should be a port") };
			// 		if self.selection_patterns[pattern].get_destination(current_router, topology, rng) == self.selection_patterns[pattern].get_destination(next_router, topology, rng)
			// 		{
			// 			return;
			// 		}
			// 		let cloned_bri= &bri.meta.as_ref().unwrap()[0];
			// 		self.default_routing.update_routing_info(cloned_bri, topology, current_router, current_port, target_router, target_server, rng);
			// 	}
			//
			// }

		}
		else
		{
			let routing_bri= &(bri.meta.as_ref().unwrap()[0]);
			self.default_routing.update_routing_info(routing_bri, topology, current_router, current_port, target_router, target_server, rng);
		}

	}

	fn initialize(&mut self, topology: &dyn Topology, rng: &mut StdRng) {
		for (i, pat) in self.physical_to_logical.iter_mut().enumerate() {
			pat.initialize(topology.num_routers(), self.selected_region_size[i], topology, rng);
			let mut physical_to_logical = vec![0; topology.num_routers()];
			for router in 0..topology.num_routers()
			{
				physical_to_logical[router] = pat.get_destination(router, topology, rng);
			}
			self.physical_to_logical_vector[i] = physical_to_logical;
		}

		for (i, pat) in self.logical_to_physical.iter_mut().enumerate() {
			pat.initialize(self.selected_region_size[i], topology.num_routers(), topology, rng);
			let mut logical_to_physical = vec![0; self.selected_region_size[i]];
			for logical_router in 0..self.selected_region_size[i]
			{
				logical_to_physical[logical_router] = pat.get_destination(logical_router, topology, rng);
			}
			self.logical_to_physical_vector[i] = logical_to_physical;
		}


		self.default_routing.initialize(topology, rng);
		for routing in self.routings.iter_mut() {
			routing.initialize(topology, rng);
		}
	}
}

impl RegionRouting
{
	pub fn new(arg: RoutingBuilderArgument) -> RegionRouting
	{
		let mut physical_to_logical = vec![];
		let mut logical_to_physical = vec![];
		let mut selected_region_size = vec![];
		let mut region_logical_topology = vec![];
		let mut routings = vec![];
		let mut default_routing = None;
		let mut extra_label_selection = 0;
		match_object_panic!(arg.cv,"RegionRouting",value,
			"physical_to_logical" => physical_to_logical = value.as_array().expect("bad value for selection_patterns").iter().map(|v|new_pattern(PatternBuilderArgument{cv:v,plugs:arg.plugs})).collect(),
			"logical_to_physical" => logical_to_physical = value.as_array().expect("bad value for map_region").iter().map(|v|new_pattern(PatternBuilderArgument{cv:v,plugs:arg.plugs})).collect(),
			"selected_region_size" => selected_region_size = value.as_array().expect("bad value for selected_size").iter().map(|v|v.as_usize().expect("bad value in selected_size")).collect(),
			"region_logical_topology" => region_logical_topology = value.as_array().expect("bad value for region_logical_topology").iter().map(|v|new_topology(TopologyBuilderArgument{cv:v,plugs:arg.plugs,rng: &mut StdRng::from_entropy()})).collect(),
			"routings" => routings = value.as_array().expect("bad value for routings").iter().map(|v|new_routing(RoutingBuilderArgument{cv:v,plugs:arg.plugs})).collect(),
			"extra_label_selection" => extra_label_selection = value.as_i32().expect("bad value for extra_label_selection"),
			"default_routing" => default_routing = Some(new_routing(RoutingBuilderArgument{cv:value,plugs:arg.plugs})),
		);
		let default_routing = default_routing.expect("missing default_routing");
		//Check that the sizes are correct
		if physical_to_logical.len() != selected_region_size.len() {
			panic!("The number of selection_patterns and selected_size must be the same.");
		}
		if physical_to_logical.len() != routings.len() {
			panic!("The number of selection_patterns and routings must be the same.");
		}
		if logical_to_physical.len() != physical_to_logical.len() {
			panic!("The number of map_region and selection_patterns must be the same.");
		}
		if region_logical_topology.len() != logical_to_physical.len() {
			panic!("The number of region_logical_topology and map_region must be the same.");
		}

		let len = physical_to_logical.len();
		RegionRouting {
			physical_to_logical,
			logical_to_physical,
			selected_region_size,
			logical_to_physical_vector: vec![vec![]; len],
			physical_to_logical_vector: vec![vec![]; len],
			region_logical_topology,
			routings,
			extra_label_selection,
			default_routing,
		}
	}
}
