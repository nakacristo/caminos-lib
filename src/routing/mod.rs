
/*!

A Routing defines the ways to select a next router to eventually reach the destination.

see [`new_routing`](fn.new_routing.html) for documentation on the configuration syntax of predefined routings.

*/

/// Contains Shortest, Valiant, Mindless, WeighedShortest.
pub mod basic;
/// Contains Sum, Stubborn, EachLengthSourceAdaptiveRouting
pub mod extra;
/// Contains ChannelsPerHop, ChannelsPerHopPerLinkClass, ChannelMap, AscendantChannelsWithLinkClass
pub mod channel_operations;
/// Contains UpDown, UpDownStar.
pub mod updown;
pub mod polarized;

use crate::topology::dragonfly::DragonflyDirect;
use std::cell::RefCell;
use std::fmt::Debug;
use std::convert::TryFrom;

use ::rand::{rngs::StdRng,Rng,prelude::SliceRandom};

use crate::config_parser::ConfigurationValue;
use crate::topology::cartesian::{DOR, O1TURN, ValiantDOR, OmniDimensionalDeroute, DimWAR, GENERALTURN, Valiant4Hamming, AdaptiveValiantClos};
use crate::topology::dragonfly::{PAR, Valiant4Dragonfly};
use crate::topology::{Topology,Location};
pub use crate::event::Time;
use quantifiable_derive::Quantifiable;//the derive macro
use crate::{Plugs};
pub use crate::error::Error;
use crate::topology::megafly::MegaflyAD;
use crate::topology::multistage::UpDownDerouting;

pub use self::basic::*;
pub use self::extra::*;
pub use self::channel_operations::*;
pub use self::updown::*;
pub use self::polarized::Polarized;

pub mod prelude
{
	pub use super::{new_routing,Routing,RoutingInfo,RoutingNextCandidates,CandidateEgress,RoutingBuilderArgument,Error,Time};
}

///Information stored in the packet for the `Routing` algorithms to operate.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct RoutingInfo
{
	///Number of edges traversed (Router--Router). It is computed by the advance routine of the simulator.
	pub hops: usize,

	//All the remaining fields are used and computed by the Routing employed.
	///Difference in coordinates from origin to destination
	pub routing_record: Option<Vec<i32>>,
	///List of router indexes in the selected path from origin to destination
	pub selected_path: Option<Vec<usize>>,
	///Some selections made by the routing
	pub selections: Option<Vec<i32>>,
	///List of router indexes that have been visited already.
	pub visited_routers: Option<Vec<usize>>,
	///Mostly for the generic Valiant scheme.
	pub meta: Option<Vec<RefCell<RoutingInfo>>>,
	///Arbitrary data with internal mutability.
	pub auxiliar: RefCell<Option<Box<dyn std::any::Any>>>,
	///Source server index, optional.
	pub source_server: Option<usize>,
}

impl RoutingInfo
{
	pub fn new() -> RoutingInfo
	{
		RoutingInfo{
			hops: 0,
			routing_record: None,
			selected_path: None,
			selections: None,
			visited_routers: None,
			meta: None,
			auxiliar: RefCell::new(None),
			source_server: None,
		}
	}
}

///Annotations by the routing to keep track of the candidates.
#[derive(Clone,Debug,Default)]
pub struct RoutingAnnotation
{
	pub(crate) values: Vec<i32>,
	pub(crate) meta: Vec<Option<RoutingAnnotation>>,
}

///Represent a port plus additional information that a routing algorithm can determine on how a packet must advance to the next router or server.
#[derive(Clone)]
#[derive(Debug,Default)]
pub struct CandidateEgress
{
	///Candidate exit port
	pub port: usize,
	///Candidate virtual channel in which being inserted.
	pub virtual_channel: usize,
	///Value used to indicate priorities. Semantics defined per routing and policy. Routing should use low values for more priority.
	pub label: i32,
	///An estimation of the number of hops pending. This include the hop we are requesting.
	pub estimated_remaining_hops: Option<usize>,

	///The routing must set this to None.
	///The `Router` can set it to `Some(true)` when it satisfies all flow-cotrol criteria and to `Some(false)` when it fails any criterion.
	pub router_allows: Option<bool>,

	///Annotations for the routing to know to what candidate the router refers.
	///It should be preserved by the policies.
	pub annotation: Option<RoutingAnnotation>,
}

impl CandidateEgress
{
	pub fn new(port:usize, virtual_channel:usize)->CandidateEgress
	{
		CandidateEgress{
			port,
			virtual_channel,
			label: 0,
			estimated_remaining_hops: None,
			router_allows: None,
			annotation: None,
		}
	}
}

///The candidates as provided by the routing together with related information.
///This is, the return type of `Routing::next`.
#[derive(Clone,Debug,Default)]
pub struct RoutingNextCandidates
{
	///The vector of candidates.
	pub candidates: Vec<CandidateEgress>,
	///Whether sucessive calls to the routing algorithm will find the exact same set of candidates.
	///If a call returns a `RoutingNextCandidates` with some value of `idempotent` then successive calls should also have that same value of `idempotent`.
	///Returning `idempotent` to false allows to change the `candidates` in another call but this field should be kept to false.
	///Setting this flag to true allows the [Router][crate::router::Router] to skip calls to the routing algorithm or even to skip some events of the router.
	pub idempotent: bool,
}

impl From<RoutingNextCandidates> for Vec<CandidateEgress>
{
	fn from(candidates: RoutingNextCandidates) -> Self
	{
		candidates.candidates
	}
}

impl IntoIterator for RoutingNextCandidates
{
	type Item = CandidateEgress;
	type IntoIter = <Vec<CandidateEgress> as IntoIterator>::IntoIter;
	fn into_iter(self) -> <Self as IntoIterator>::IntoIter
	{
		self.candidates.into_iter()
	}
}

impl RoutingNextCandidates
{
	pub fn len(&self)->usize
	{
		self.candidates.len()
	}
}

///A routing algorithm to provide candidate routes when the `Router` requires.
///It may store/use information in the RoutingInfo.
///A `Routing` does not receive information about the state of buffers or similar. Such a mechanism should be given as a `VirtualChannelPolicy`.
pub trait Routing : Debug
{
	/// Compute the list of allowed exits.
	/// `routing_info` contains the information in the packet being routed.
	/// `current_router` is the index of the router in the `topology` that is performing the routing.
	/// `target_router` is the index of the router towards which we are routing.
	/// If `target_server` is not None it is the server destination of the packet, which must be attached to `target_router`. A routing that works without this can be more simply used as part of other routing algorithms, as it may be used to route to intermediate routers even on indirect topologies.
	/// `num_virtual_channels` is the number of virtual channels dedicated to this routing.
	/// `rng` is the global generator of random numbers.
	fn next(&self, routing_info:&RoutingInfo, topology:&dyn Topology, current_router:usize, target_router:usize, target_server:Option<usize>, num_virtual_channels:usize, rng: &mut StdRng) -> Result<RoutingNextCandidates,Error>;
	//fn initialize_routing_info(&self, routing_info:&mut RoutingInfo, topology:&dyn Topology, current_router:usize, target_server:usize);
	///Initialize the routing info of the packet. Called when the first phit of the packet leaves the server and enters a router.
	fn initialize_routing_info(&self, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_touter:usize, _target_server:Option<usize>, _rng: &mut StdRng) {}
	///Updates the routing info of the packet. Called when the first phit of the packet leaves a router and enters another router. Values are of the router being entered into.
	fn update_routing_info(&self, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _current_port:usize, _target_router:usize, _target_server:Option<usize>,_rng: &mut StdRng) {}
	///Prepares the routing to be utilized. Perhaps by precomputing routing tables.
	fn initialize(&mut self, _topology:&dyn Topology, _rng: &mut StdRng) {}
	///To be called by the router when one of the candidates is requested.
	fn performed_request(&self, _requested:&CandidateEgress, _routing_info:&RefCell<RoutingInfo>, _topology:&dyn Topology, _current_router:usize, _target_router:usize, _target_server:Option<usize>, _num_virtual_channels:usize, _rng:&mut StdRng) {}
	///To optionally write routing statistics into the simulation output.
	fn statistics(&self,_cycle:Time) -> Option<ConfigurationValue>{ None }
	///Clears all collected statistics
	fn reset_statistics(&mut self,_next_cycle:Time) {}
}

///The argument of a builder function for `Routings`.
#[derive(Debug)]
pub struct RoutingBuilderArgument<'a>
{
	///A ConfigurationValue::Object defining the routing.
	pub cv: &'a ConfigurationValue,
	///The user defined plugs. In case the routing needs to create elements.
	pub plugs: &'a Plugs,
}

/**Build a new routing.

## Generic routings

The most basic routing is to use any shortest path from the source to the destination. Note that for many traffic patterns using only these paths
make the packets flow through a few links, with enormous congestion.

```ignore
Shortest{
	legend_name: "minimal routing",
}
```

As solution for those cases problematic for shortest routing, Valiant proposed a randomization scheme. Each packet to be sent from a source to a destination is routed first to a random intermediate node, and from that intermediate to destination. These randomization makes the two parts behave as if the
traffic pattern was uniform at the cost of doubling the lengths.

See Valiant, L. G. (1982). A scheme for fast parallel communication. SIAM journal on computing, 11(2), 350-361.

```ignore
Valiant{
	first: Shortest,
	second: Shortest,
	legend_name: "Using Valiant scheme, shortest to intermediate and shortest to destination",
	//selection_exclude_indirect_routers: false,//optional parameter
}
```

As a routing that gives both short, long routes, and many intermediates we have the Polarized routing. It is recommended to have some mechanism to select among those routes based on network measures such as queue occupation.

- Camarero, C., Martínez, C., & Beivide, R. (2021, August). Polarized routing: an efficient and versatile algorithm for large direct networks. In 2021 IEEE Symposium on High-Performance Interconnects (HOTI) (pp. 52-59). IEEE.
- Camarero, C., Martínez, C., & Beivide, R. (2022). Polarized routing for large interconnection networks. IEEE Micro, 42(2), 61-67.

```ignore
Polarized{
	/// Include the weight as label, shifted so that the lowest weight is given the label 0. Otherwise it just put a value of 0 for all.
	include_labels: true,
	/// Restrict the routes to those that strictly improve the weight function at each step.
	/// Note that mmany/most topologies benefit from using routes that have a few edges with no change to the weight.
	/// Therefore one should expect too few routes when using this option.
	/// Strong polarized routes have maximum length of at most 2*diameter.
	strong: false,
	/// Whether to raise a panic when there are no candidates. default to true.
	/// It is to be set to false when employing in conjunction with another routing when Polarized return an empty set of routes.
	//panic_on_empty: true,
	/// Builds a `PolarizedStatistics{empty_count:XX,best_count:[XX,YY,ZZ]}` in the results.
	/// It tracks the number of first calls to `next` that returned an empty set and the number of times the best candidate was either +0, +1, or +2.
	/// defaults to false.
	//enable_statistics: false,
	/// Its name in generated plots.
	legend_name: "Polarized routing",
}
```

For topologies that define global links:
```ignore
WeighedShortest{
	class_weight: [1,100],
	legend_name: "Shortest avoiding using several global links",
}
```

For multi-stage topologies we may use
```ignore
UpDown{
	legend_name: "up/down routing",
}
```

There is a `Mindless` routing without parameters that includes all neighbours as candidates until reaching destination. Can be though as a random walk, if additionally the router would make its decisions randomly.

## Operations

### Sum
To use some of two routings depending on whatever. virtual channels not on either list can be used freely. The extra label field can be used to set the priorities. Check the router policies for that.
```ignore
Sum{
	policy: TryBoth,//or Random
	first_routing: Shortest,
	second_routing: Valiant{first:Shortest,second:Shortest},
	first_allowed_virtual_channels: [0,1],
	second_allowed_virtual_channels: [2,3,4,5],
	first_extra_label:0,//optional
	second_extra_label:10,//optiona
	legend_name: "minimal with high priority and Valiant with low priority",
}
```

### ChannelsPerHop
Modify a routing to use a given list of virtual channels each hop.
```ignore
ChannelsPerHop{
	routing: Shortest,
	channels: [
		[0],//the first hop from a router to another router
		[1],
		[2],
		[0,1,2],//the last hop, to the server
	],
}
```

### ChannelsPerHopPerLinkClass
Modify a routing to use a given list of virtual channels each hop.
```ignore
ChannelsPerHopPerLinkClass{
	routing: Shortest,
	channels: [
		[ [0],[1] ],//links in class 0.
		[ [0],[1] ],//links in class 1.
		[ [0,1] ],//links in class 2. Last class is towards servers. 
	],
}
```

### ChannelMap
```ignore
ChannelMap{
	routing: Shortest,
	map: [
		[1],//map the virtual channel 0 into the vc 1
		[2,3],//the vc 1 is doubled into 2 and 3
		[4],
	],
}
```

### AscendantChannelsWithLinkClass
Virtual channels are used in ascent way. With higher classes meaning higher digits.
```ignore
AscendantChannelsWithLinkClass{
	routing: Shortest,
	bases: [2,1],//allow two consecutive hops of class 0 before a hop of class 1
}
```

### Stubborn makes a routing to calculate candidates just once. If that candidate is not accepted is trying again every cycle.
```ignore
Stubborn{
	routing: Shortest,
	legend_name: "stubborn minimal",
}
```

## Cartesian-specific routings

### DOR

The dimensional ordered routing. Packets will go minimal along the first dimension as much possible and then on the next.

```ignore
DOR{
	order: [0,1],
	legend_name: "dimension ordered routing, 0 before 1",
}
```


### O1TURN
O1TURN is a pair of DOR to balance the usage of the links.

```ignore
O1TURN{
	reserved_virtual_channels_order01: [0],
	reserved_virtual_channels_order10: [1],
	legend_name: "O1TURN",
}
```

### OmniDimensional

McDonal OmniDimensional routing for HyperX. it is a shortest with some allowed deroutes. It does not allow deroutes on unaligned dimensions.

```ignore
OmniDimensionalDeroute{
	allowed_deroutes: 3,
	include_labels: true,//deroutes are given higher labels, implying lower priority. Check router policies.
	legend_name: "McDonald OmniDimensional routing allowing 3 deroutes",
}
```

### ValiantDOR

A proposal by Valiant for Cartesian topologies. It randomizes all-but-one coordinates, followed by a DOR starting by the non-randomized coordinate.

```ignore
ValiantDOR{
	randomized: [2,1],
	shortest: [0,1,2],
	randomized_reserved_virtual_channels: [1],
	shortest_reserved_virtual_channels: [0],
	legend_name: "The less-known proposal of Valiant for Cartesian topologies",
}
```

*/
pub fn new_routing(arg: RoutingBuilderArgument) -> Box<dyn Routing>
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
	{
		match arg.plugs.routings.get(cv_name)
		{
			Some(builder) => return builder(arg),
			_ => (),
		};
		match cv_name.as_ref()
		{
			"DOR" => Box::new(DOR::new(arg)),
			"O1TURN" => Box::new(O1TURN::new(arg)),
			"GeneralTurn" => Box::new(GENERALTURN::new(arg)),
			"OmniDimensionalDeroute" => Box::new(OmniDimensionalDeroute::new(arg)),
			"DimWAR" => Box::new(DimWAR::new(arg)),
			"Valiant4Hamming" => Box::new(Valiant4Hamming::new(arg)),
			"AdaptiveValiantClos" => Box::new(AdaptiveValiantClos::new(arg)),
			"Valiant4Dragonfly" => Box::new(Valiant4Dragonfly::new(arg)),
			"PAR" => Box::new(PAR::new(arg)),
			"Shortest" => Box::new(Shortest::new(arg)),
			"Valiant" => Box::new(Valiant::new(arg)),
			"ValiantDOR" => Box::new(ValiantDOR::new(arg)),
			"Polarized" => Box::new(Polarized::new(arg)),
			"Sum" => Box::new(SumRouting::new(arg)),
			"Mindless" => Box::new(Mindless::new(arg)),
			"WeighedShortest" => Box::new(WeighedShortest::new(arg)),
			"Stubborn" => Box::new(Stubborn::new(arg)),
			"UpDown" => Box::new(UpDown::new(arg)),
			"UpDownStar" => Box::new(ExplicitUpDown::new(arg)),
			"ChannelsPerHop" => Box::new(ChannelsPerHop::new(arg)),
			"ChannelsPerHopPerLinkClass" => Box::new(ChannelsPerHopPerLinkClass::new(arg)),
			"AscendantChannelsWithLinkClass" => Box::new(AscendantChannelsWithLinkClass::new(arg)),
			"ChannelMap" => Box::new(ChannelMap::new(arg)),
			"Dragonfly2Colors" => Box::new(crate::topology::dragonfly::Dragonfly2ColorsRouting::new(arg)),
			"UpDownDerouting" => Box::new(UpDownDerouting::new(arg)),
			"MegaflyAD" => Box::new(MegaflyAD::new(arg)),
			"AdaptiveStart" => Box::new(AdaptiveStart::new(arg)),
			"DragonflyDirect" => Box::new(DragonflyDirect::new(arg)),
			"SubTopologyRouting" => Box::new(SubTopologyRouting::new(arg)),
			"RegionRouting" => Box::new(RegionRouting::new(arg)),
			_ => panic!("Unknown Routing {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create a Routing from a non-Object");
	}
}


///Trait for `Routing`s that build the whole route at source.
///This includes routings such as K-shortest paths. But I have all my implementations depending on a private algorithm, so they are not yet here.
///They will all be released when the dependency is formally published.
pub trait SourceRouting
{
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng);
	fn get_paths(&self, source:usize, target:usize) -> &Vec<Vec<usize>>;
}

pub trait InstantiableSourceRouting : SourceRouting + Debug {}
impl<R:SourceRouting + Debug> InstantiableSourceRouting for R {}

impl<R:SourceRouting+Debug> Routing for R
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
						return Ok(RoutingNextCandidates{candidates:(0..num_virtual_channels).map(|vc|CandidateEgress::new(i,vc)).collect(),idempotent:true});
					}
				}
			}
			unreachable!();
		}
		let num_ports=topology.ports(current_router);
		let mut r=Vec::with_capacity(num_ports*num_virtual_channels);
		let next_router=routing_info.selected_path.as_ref().unwrap()[routing_info.hops+1];
		let length =routing_info.selected_path.as_ref().unwrap().len() - 1;//substract source router
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
						egress
					}));
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
		if current_router!=target_router
		{
			//let path_collection = &self.paths[current_router][target_router];
			let path_collection = self.get_paths(current_router,target_router);
			//println!("path_collection.len={} for source={} target={}\n",path_collection.len(),current_router,target_router);
			if path_collection.is_empty()
			{
				panic!("No path found from router {} to router {}",current_router,target_router);
			}
			let r=rng.gen_range(0..path_collection.len());
			routing_info.borrow_mut().selected_path=Some(path_collection[r].clone());
		}
	}
	fn initialize(&mut self, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.initialize(topology,rng);
	}
}






///Encapsulation of SourceRouting, to allow storing several paths in the packet. And then, have adaptiveness for the first hop.
#[derive(Debug)]
pub struct SourceAdaptiveRouting
{
	///The base routing
	pub routing: Box<dyn InstantiableSourceRouting>,
	///Maximum amount of paths to store
	pub amount: usize,
}

impl Routing for SourceAdaptiveRouting
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
			let mut selected_indices : Vec<i32> = (0i32..<i32>::try_from(path_collection.len()).unwrap()).collect();
			if selected_indices.len()>self.amount
			{
				//rng.borrow_mut().shuffle(&mut selected_indices);//rand-0.4
				//selected_indices.shuffle(rng.borrow_mut().deref_mut());
				selected_indices.shuffle(rng);
				selected_indices.resize_with(self.amount,||unreachable!());
			}
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







