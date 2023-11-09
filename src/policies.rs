/*!

A policy defined to what output queue/port to request among the ones returned as possible by the routing function. Policies are designed to be applied in sequence so that there remains at most a single candidate.

One should include always the policy `EnforceFlowControl` or equivalent at some point. To ensure at most one candidate you may use the `Random` policy.

see [`new_virtual_channel_policy`](fn.new_virtual_channel_policy.html) for documentation on the configuration syntax of predefined policies.

*/

use crate::config_parser::ConfigurationValue;
use crate::routing::CandidateEgress;
use crate::router::Router;
use crate::topology::{Topology,Location};
use crate::{Plugs,Phit,match_object_panic};
use crate::event::Time;

use std::fmt::Debug;
use std::convert::TryInto;
use std::rc::Rc;

use ::rand::{Rng,rngs::StdRng};

///Extra information to be used by the policies of virtual channels.
#[derive(Debug)]
pub struct RequestInfo<'a>
{
	///target_router_index: The index of the router to which the destination server is attached.
	pub target_router_index: usize,
	///entry_port: The port for which the packet has entered into the current router.
	pub entry_port: usize,
	///entry_virtual_channel: The virtual_channel the packet used when it entered into the current router.
	pub entry_virtual_channel: usize,
	///performed_hops: the amount of hops already made by the packet.
	pub performed_hops: usize,
	///server_ports: a list of which ports from the current router go to server.
	pub server_ports: Option<&'a Vec<usize>>,
	///port_average_neighbour_queue_length: for each port the average queue length in the queues of the port in the neighbour router.
	pub port_average_neighbour_queue_length: Option<&'a Vec<f32>>,
	///port_last_transmission: a timestamp for each port of the last time that it was used.
	pub port_last_transmission: Option<&'a Vec<Time>>,
	///Number of phits currently in the output space of the current router at the indexed port.
	pub port_occupied_output_space: Option<&'a Vec<usize>>,
	///Number of available phits in the output space of the current router at the indexed port.
	pub port_available_output_space: Option<&'a Vec<usize>>,
	///Number of phits currently in the output space allocated to a virtual channel. Index by `[port_index][virtual_channel]`.
	pub virtual_channel_occupied_output_space: Option<&'a Vec<Vec<usize>>>,
	///Number of available phits in the output space allocated to a virtual channel. Index by `[port_index][virtual_channel]`.
	pub virtual_channel_available_output_space: Option<&'a Vec<Vec<usize>>>,
	///Number of cycles at the front of input space,
	pub time_at_front: Option<usize>,
	///current_cycle: The current cycle of the simulation.
	pub current_cycle: Time,
	///The phit for which we are requesting an egress.
	pub phit: Rc<Phit>,
}

///How virtual channels are selected for a packet
///They provide the function `filter(Vec<CandidateEgress>) -> Vec<CandidateEgress>`
///It needs:
///	rng, self.virtual_ports(credits and length), phit.packet.routing_info.borrow().hops, server_ports,
/// topology.{distance,neighbour}, port_average_neighbour_queue_length, port_last_transmission
///We could also provide functions to declare which aspects must be computed. Thus allowing to both share when necessary and to not computing ti when unnecessary.
pub trait VirtualChannelPolicy : Debug
{
	///Apply the policy over a list of candidates and return the candidates that fulfil the policy requirements.
	///candidates: the list to be filtered.
	///router: the router in which the decision is being made.
	///topology: The network topology.
	///rng: the global random number generator.
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, topology:&dyn Topology, rng: &mut StdRng) -> Vec<CandidateEgress>;
	fn need_server_ports(&self)->bool;
	fn need_port_average_queue_length(&self)->bool;
	fn need_port_last_transmission(&self)->bool;
}

#[derive(Debug)]
pub struct VCPolicyBuilderArgument<'a>
{
	///A ConfigurationValue::Object defining the policy.
	pub cv: &'a ConfigurationValue,
	///The user defined plugs. In case the policy needs to create elements.
	pub plugs: &'a Plugs,
}

/** Build a new virtual channel policy. These policies are successive refinements over the available ones as returned by the routing function, to determine to which exit a request is done.

## Basic policies

### Identity

No operation. To be used inside meta-policies that can switch among several others.

### EnforceFlowControl

Filter out those candidates that have not enough credits or the equivalent depending on buffer structure. It should appear at least once in the list of policies.

### Random

Selects one candidate randomly among the available.

### Shortest

Selects the port+virtual channel with more available credits. Do not resolve ties.


### Hops

A policy to avoid deadlock. Use the virtual channel `i` in the `i`-th hop of the packet.

### WideHops

A variant of Hops that allow `width` VCs in each hop.

```ignore
WideHops{ width:2 }
```

## Policies measuring queues

### LowestSinghWeight

Select the lowest value of the product of the queue length (that is, consumed credits) times the estimated hop count (usually 1 plus the distance from next router to target router)
This was initially proposed for the UGAL routing.

```ignore
LowestSinghWeight
{
	//We may add a small constant so the distance is always relevant, even for low loads
	extra_congestion: 1,
	//For distances is less important to add anything.
	extra_distance: 0,
	//Whether to aggregate all buffers in that port or to use just the space of the candidate.
	aggregate: false,
	///Whether to include in the computation the space of the output queue.
	use_internal_space: true
	///Whether to include in the computation the space in the neighbour router. It uses a credit counter as proxy.
	use_neighbour_space: true,
	///Whether to use the estimation of remaining hops given by the routing algorithm.
	///Some non-minimal routing may provide that estimation, check their documentation.
	use_estimation: true,
}
```

### OccupancyFunction

## Label manipulation

Some routings can label their candidates. For example into minimal/non-minimal routes. We may use that classification to make decisions on them.

### LowestLabel

Select the candidate with least label (possible signifying minimal routing).

### LabelSaturate

Apply the transformation `New label = min{old_label,value} or max{old_label,value}`

```ignore
LabelSaturate
{
	value: 1,
	bottom: true,
}
```

### LabelTransform

More advanced transformations. Lineal operation with optional saturation (reducing/raising the value) and limits (filtering out).

```ignore
LabelTransform
{
	multiplier: 1,
	summand: 1,
	saturate_bottom: 0,
	//saturate_top
	//minimum
	maximum: 2,
}
```

### NegateLabel

Negate the label. Alternatively use `LabelTransform` with `multiplier:-1`.

### VecLabel

Apply a map to the label, i.e., `new_label = vector[old_label]`.

```ignore
VecLabel
{
	label_vector: [1, 0],
}
```

### MapLabel

A meta-policy applying a different policy to candidates with each label.

```ignore
MapLabel
{
	//Apply Identity to label 0.
	//Apply Random to label 1.
	label_to_policy: [Identity, Random],
	//We may apply a policy to negative labels
	//below_policy: Identity,
	//We may apply a policy to label values over the range
	//above_policy: Identity,
}
```

## Purely VC transformations

### ShiftEntryVC

Only allows those candidates whose vc equals their entry vc plus some `s` in `shifts`. This is very similar to the `Hops` policy, but can be combined with other policies. For example, to increase VC only in a escape sub-network.

```ignore
ShiftEntryVC
{
	shifts: [1],
}
```

### ArgumentVC

Only allows those candidates whose vc is in the allowed list. To be used inside meta-policies.

```ignore
ArgumentVC
{
	allowed: [0, 1]
}
```

### MapEntryVC

A meta-policy applying a different policy to candidates from each entry virtual channel.

```ignore
MapEntryVC
{
	//Do nothing over the two first VCs
	vc_to_policy: [Identity, Identity]
	//In the other VCs the packet must increase each hop.
	above_policy: ShiftEntryVC{shifts:[1]},
}
```

## Hop based

Policies that use the number of hops given by the packet. We have already commented on `Hops` and `WideHops`.

### MapHop

Meta-policy applying a different policy to each hop.

```ignore
MapHop
{
	//First hop from a router to another must be in VC 0 or 1.
	hop_to_policy: [ArgumentVC{allowed:[0,1]}],
	//Further hops increase the VC number by 1.
	above_policy: ShiftEntryVC{shifts:[1]},
}
```

### VOQ

Employ a different VC (or policy) to each destination.

Example configuration:
```ignore
VOQ{
	/// Optionally set a number of VCs to use in this policy. By default it uses a VC per destination node.
	/// Packets to destination `dest` will use VC number `(dest % num_classes) + start_virtual_channel`.
	//num_classes: 4,
	/// Optionally, use the index of the destination switch instead of the destination server.
	switch_level: true,
	/// Optionally, give specific policies for matching indices instead of just just such index as VC.
	/// If this example had `num_classes=2`, then it would use the Identity policy for even destinations and the Hops policy for odd destinations.
	/// It can be though as having a default of infinite array full of ArgumentVC whose argument equal to the array index.
	// policies_override: [Identity,Hops],
}
```

*/
pub fn new_virtual_channel_policy(arg:VCPolicyBuilderArgument) -> Box<dyn VirtualChannelPolicy>
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
	{
		if let Some(builder) = arg.plugs.policies.get(cv_name)
		{
			return builder(arg);
		}
		match cv_name.as_ref()
		{
			"Identity" => Box::new(Identity::new(arg)),
			"Random" => Box::new(Random::new(arg)),
			"Shortest" => Box::new(Shortest::new(arg)),
			"Hops" => Box::new(Hops::new(arg)),
			"EnforceFlowControl" => Box::new(EnforceFlowControl::new(arg)),
			"WideHops" => Box::new(WideHops::new(arg)),
			"LowestSinghWeight" => Box::new(LowestSinghWeight::new(arg)),
			"LowestLabel" => Box::new(LowestLabel::new(arg)),
			"LabelSaturate" => Box::new(LabelSaturate::new(arg)),
			"LabelTransform" => Box::new(LabelTransform::new(arg)),
			"OccupancyFunction" => Box::new(OccupancyFunction::new(arg)),
			"AverageOccupancyFunction" => Box::new(AverageOccupancyFunction::new(arg)),
			"NegateLabel" => Box::new(NegateLabel::new(arg)),
			"VecLabel" => Box::new(VecLabel::new(arg)),
			"MapLabel" => Box::new(MapLabel::new(arg)),
			"ShiftEntryVC" => Box::new(ShiftEntryVC::new(arg)),
			"MapHop" => Box::new(MapHop::new(arg)),
			"ArgumentVC" => Box::new(ArgumentVC::new(arg)),
			"Either" => Box::new(Either::new(arg)),
			"MapEntryVC" => Box::new(MapEntryVC::new(arg)),
			"MapMessageSize" => Box::new(MapMessageSize::new(arg)),
			"Chain" => Box::new(Chain::new(arg)),
			"VOQ" => Box::new(VOQ::new(arg)),
			_ => panic!("Unknown policy {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create a policy from a non-Object\narg={:?}",arg);
	}
}

///Does not do anything. Just a placeholder for some operations.
#[derive(Debug)]
pub struct Identity{}

impl VirtualChannelPolicy for Identity
{
	fn filter(&self, candidates:Vec<CandidateEgress>, _router:&dyn Router, _info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		candidates
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}
}

impl Identity
{
	pub fn new(_arg:VCPolicyBuilderArgument) -> Identity
	{
		Identity{}
	}
}


///Request a port+virtual channel at random from all available.
#[derive(Debug)]
pub struct Random{}

impl VirtualChannelPolicy for Random
{
	//fn filter(&self, candidates:Vec<CandidateEgress>, _router:&dyn Router, _target_router_index:usize, _entry_port:usize, _entry_virtual_channel:usize, _performed_hops:usize, _server_ports:&Option<Vec<usize>>, _port_average_neighbour_queue_length:&Option<Vec<f32>>, _port_last_transmission:&Option<Vec<usize>>, _port_occuped_output_space:&Option<Vec<usize>>, _port_available_output_space:&Option<Vec<usize>>, _current_cycle:usize, _topology:&dyn Topology, rng: &mut StdRng) -> Vec<CandidateEgress>
	fn filter(&self, candidates:Vec<CandidateEgress>, _router:&dyn Router, _info: &RequestInfo, _topology:&dyn Topology, rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		vec![candidates[rng.gen_range(0..candidates.len())].clone()]
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}
}

impl Random
{
	pub fn new(_arg:VCPolicyBuilderArgument) -> Random
	{
		Random{}
	}
}

///Request the port+virtual channel with more credits. Does not solve ties, so it needs to be followed by Random or something.
#[derive(Debug)]
pub struct Shortest{}

impl VirtualChannelPolicy for Shortest
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, _info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		let mut best=vec![];
		let mut best_credits=0;
		//for i in 1..vps.len()
		for candidate in candidates.into_iter()
		{
			let CandidateEgress{port:p,virtual_channel:vc,..}=candidate;
			//let next_credits=router.virtual_ports[p][vc].neighbour_credits;
			//let next_credits=router.get_virtual_port(p,vc).expect("This router does not have virtual ports (and not credits therefore)").neighbour_credits;
			let next_credits=router.get_status_at_emisor(p).expect("This router does not have transmission status").known_available_space_for_virtual_channel(vc).expect("remote available space is not known");
			if next_credits>best_credits
			{
				best_credits=next_credits;
				//best=vec![CandidateEgress{port:p,virtual_channel:vc,label,estimated_remaining_hops}];
				best=vec![candidate];
			}
			else if next_credits==best_credits
			{
				//best.push(CandidateEgress{port:p,virtual_channel:vc,label,estimated_remaining_hops});
				best.push(candidate);
			}
		}
		best
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl Shortest
{
	pub fn new(_arg:VCPolicyBuilderArgument) -> Shortest
	{
		Shortest{}
	}
}


///Select virtual channel=packet.hops.
#[derive(Debug)]
pub struct Hops{}

impl VirtualChannelPolicy for Hops
{
	fn filter(&self, candidates:Vec<CandidateEgress>, _router:&dyn Router, info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		let server_ports=info.server_ports.expect("server_ports have not been computed for policy Hops");
		let filtered=candidates.into_iter().filter(|&CandidateEgress{port,virtual_channel,label:_label,estimated_remaining_hops:_,..}|virtual_channel==info.performed_hops|| server_ports.contains(&port)).collect::<Vec<_>>();
		//let filtered=candidates.iter().filter_map(|e|if e.1==performed_hops{Some(*e)}else {None}).collect::<Vec<_>>();
		//if filtered.len()==0
		//{
		//	//panic!("There is no route from router {} to server {} increasing on virtual channels",self.router_index,phit.packet.message.destination);
		//	continue;
		//}
		//filtered[simulation.rng.borrow_mut().gen_range(0..filtered.len())]
		filtered
	}

	fn need_server_ports(&self)->bool
	{
		true
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl Hops
{
	pub fn new(_arg:VCPolicyBuilderArgument) -> Hops
	{
		Hops{}
	}
}

///
#[derive(Debug)]
pub struct EnforceFlowControl{}

impl VirtualChannelPolicy for EnforceFlowControl
{
	fn filter(&self, candidates:Vec<CandidateEgress>, _router:&dyn Router, _info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		let filtered=candidates.into_iter().filter(|candidate|candidate.router_allows.unwrap_or(true)).collect::<Vec<_>>();
		filtered
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl EnforceFlowControl
{
	pub fn new(_arg:VCPolicyBuilderArgument) -> EnforceFlowControl
	{
		EnforceFlowControl{}
	}
}


///Select virtual channel in (width*packet.hops..width*(packet.hops+1)).
#[derive(Debug)]
pub struct WideHops{
	width:usize,
}

impl VirtualChannelPolicy for WideHops
{
	fn filter(&self, candidates:Vec<CandidateEgress>, _router:&dyn Router, info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		let server_ports=info.server_ports.expect("server_ports have not been computed for policy WideHops");
		let lower_limit = self.width*info.performed_hops;
		let upper_limit = self.width*(info.performed_hops+1);
		let filtered=candidates.into_iter().filter(
			|&CandidateEgress{port,virtual_channel,label:_,estimated_remaining_hops:_,..}| (lower_limit<=virtual_channel && virtual_channel<upper_limit) || server_ports.contains(&port)
		).collect::<Vec<_>>();
		//let filtered=candidates.iter().filter_map(|e|if e.1==info.performed_hops{Some(*e)}else {None}).collect::<Vec<_>>();
		//if filtered.len()==0
		//{
		//	//panic!("There is no route from router {} to server {} increasing on virtual channels",self.router_index,phit.packet.message.destination);
		//	continue;
		//}
		//filtered[simulation.rng.borrow_mut().gen_range(0..filtered.len())]
		filtered
	}

	fn need_server_ports(&self)->bool
	{
		true
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl WideHops
{
	pub fn new(arg:VCPolicyBuilderArgument) -> WideHops
	{
		let mut width=None;
		match_object_panic!(arg.cv,"WideHops",value,
			"width" => width = Some(value.as_f64().expect("bad value for width") as usize),
		);
		let width=width.expect("There were no width");
		WideHops{
			width
		}
	}
}

///Select the lowest value of the product of the queue length (that is, consumed credits) times the estimated hop count (usually 1 plus the distance from next router to target router)
///This was initially proposed for the UGAL routing.
///parameters=(extra_congestion,extra_distance,aggregate_buffers), which are added in the formula to allow tuning. Firth two default to 0.
///aggregate_buffers indicates to use all buffers instead of just the selected one.
#[derive(Debug)]
pub struct LowestSinghWeight
{
	///constant added to the occupied space
	extra_congestion: usize,
	///constant added to the distance to target
	extra_distance: usize,
	///Whether we consider all the space in each port (when true) or we segregate by virtual channels (when false).
	///defaults to false
	///Previously called aggregate_buffers
	aggregate: bool,
	///Whether to use internal output space in the calculations instead of the counters relative to the next router.
	///defaults to false
	use_internal_space: bool,
	///Whether to add the neighbour space.
	///Defaults to true.
	use_neighbour_space: bool,
	///Try `estimated_remaining_hops` before calling distance
	use_estimation: bool,
}

impl VirtualChannelPolicy for LowestSinghWeight
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=info.port_average_neighbour_queue_length.expect("port_average_neighbour_queue_length have not been computed for policy LowestSinghWeight");
		let dist=topology.distance(router.get_index().expect("we need routers with index"),info.target_router_index);
		if dist==0
		{
			//do nothing
			candidates
		}
		else
		{
			let mut best=vec![];
			//let mut best_weight=<usize>::max_value();
			let mut best_weight=<i32>::MAX;
			//let mut best_weight=::std::f32::MAX;
			//for i in 0..candidates.len()
			//for CandidateEgress{port:p,virtual_channel:vc,label,estimated_remaining_hops} in candidates
			for candidate in candidates
			{
				let CandidateEgress{port:p,virtual_channel:vc, estimated_remaining_hops, ..} = candidate;
				//let next_consumed_credits:f32=(self.extra_congestion as f32)+if self.aggregate_buffers
				//{
				//	if self.use_internal_space
				//	{
				//		let port_occupied_output_space=info.port_occupied_output_space.expect("port_occupied_output_space have not been computed for policy LowestSinghWeight");
				//		port_occupied_output_space[p] as f32
				//	}
				//	else
				//	{
				//		port_average_neighbour_queue_length[p]
				//	}
				//}
				//else
				//{
				//	if self.use_internal_space
				//	{
				//		unimplemented!()
				//	}
				//	else
				//	{
				//		//(router.buffer_size - router.virtual_ports[p][vc].neighbour_credits) as f32
				//		let next_credits=router.get_status_at_emisor(p).expect("This router does not have transmission status").known_available_space_for_virtual_channel(vc).expect("remote available space is not known");
				//		(router.get_maximum_credits_towards(p,vc).expect("we need routers with maximum credits") - next_credits) as f32
				//	}
				//};
				let q:i32 = (self.extra_congestion as i32) + if self.use_internal_space
				{
					if self.aggregate
					{
						let port_occupied_output_space=info.port_occupied_output_space.expect("port_occupied_output_space have not been computed for policy LowestSinghWeight");
						port_occupied_output_space[p] as i32
					}
					else
					{
						let virtual_channel_occupied_output_space=info.virtual_channel_occupied_output_space.expect("virtual_channel_occupied_output_space have not been computed for LowestSinghWeight");
						virtual_channel_occupied_output_space[p][vc] as i32
					}
				}
				else {0} + if self.use_neighbour_space
				{
					if self.aggregate
					{
						//port_average_neighbour_queue_length[p]
						let status=router.get_status_at_emisor(p).expect("This router does not have transmission status");
						//FIXME: this could be different than the whole occuped space if using a DAMQ or something, although they are yet to be implemented.
						(0..status.num_virtual_channels()).map(|c|router.get_maximum_credits_towards(p,c).expect("we need routers with maximum credits") as i32 - status.known_available_space_for_virtual_channel(c).expect("remote available space is not known.") as i32).sum()
					}
					else
					{
						//port_average_neighbour_queue_length[p]
						let status=router.get_status_at_emisor(p).expect("This router does not have transmission status");
						router.get_maximum_credits_towards(p,vc).expect("we need routers with maximum credits") as i32 - status.known_available_space_for_virtual_channel(vc).expect("remote available space is not known.") as i32
					}
				}
				else {0};
				let next_router=if let (Location::RouterPort{router_index, router_port:_},_link_class)=topology.neighbour(router.get_index().expect("we need routers with index"),p)
				{
					router_index
				}
				else
				{
					panic!("We trying to go to the server when we are at distance {} greater than 0.",dist);
				};
				//let distance=self.extra_distance + 1+topology.distance(next_router,info.target_router_index);
				let distance = self.extra_distance + if let (true,Some(d)) = (self.use_estimation,estimated_remaining_hops) {
					d
				} else {
					1 + topology.distance(next_router,info.target_router_index)
				};
				let next_weight= q * (distance as i32);
				if next_weight<best_weight
				{
					best_weight=next_weight;
					//best=vec![CandidateEgress{port:p,virtual_channel:vc,label,estimated_remaining_hops}];
					best=vec![candidate];
				}
				else if next_weight==best_weight
				{
					//best.push(CandidateEgress{port:p,virtual_channel:vc,label,estimated_remaining_hops});
					best.push(candidate);
				}
			}
			best
		}
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		//We have removed it. Now it uses router.get_status_at_emisor
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl LowestSinghWeight
{
	pub fn new(arg:VCPolicyBuilderArgument) -> LowestSinghWeight
	{
		let mut extra_congestion=None;
		let mut extra_distance=None;
		let mut aggregate=false;
		let mut use_internal_space=false;
		let mut use_neighbour_space=true;
		let mut use_estimation=true;
		match_object_panic!(arg.cv,"LowestSinghWeight",value,
			"extra_congestion" => extra_congestion = Some(value.as_f64().expect("bad value for extra_congestion") as usize),
			"extra_distance" => extra_distance= Some(value.as_f64().expect("bad value for extra_distance") as usize),
			"aggregate" => aggregate = value.as_bool().expect("bad value for aggregate"),
			"aggregate_buffers" => {
				println!("WARNING: the name `aggregate_buffers` has been deprecated in favour of just `aggregate`");
				aggregate = value.as_bool().expect("bad value for aggregate_buffers");
			},
			"use_internal_space" => use_internal_space = value.as_bool().expect("bad value for use_internal_space"),
			"use_neighbour_space" => use_neighbour_space = value.as_bool().expect("bad value for use_neighbour_space"),
			"use_estimation" => use_estimation = value.as_bool().expect("bad value for use_estimation"),
		);
		let extra_congestion=extra_congestion.unwrap_or(0);
		let extra_distance=extra_distance.unwrap_or(0);
		LowestSinghWeight{
			extra_congestion,
			extra_distance,
			aggregate,
			use_internal_space,
			use_neighbour_space,
			use_estimation,
		}
	}
}


///Transform (l,q) into new label a*l+b*q+c*l*q+d
///where l is the label and q is the occupancy.
///
#[derive(Debug)]
pub struct AverageOccupancyFunction
{
	///Which multiplies the label.
	label_coefficient: i32,
	///Which multiplies the occupancy.
	occupancy_coefficient: i32,
	///Which multiplies the product of label and occupancy.
	product_coefficient: i32,
	///Just added.
	constant_coefficient: i32,
	///Whether to use internal output space in the calculations instead of the counters relative to the next router.
	///defaults to false
	use_internal_space: bool,
	///Whether to add the neighbour space.
	///Defaults to true.
	use_neighbour_space: bool,
	///Virtual channels we are interested in.
	virtual_channels: Vec<usize>,
	///Whether to average the occupation of the virtual channels or add them.
	average_virtual_channels: bool,
	///Whether to exclude minimal port from the calculations.
	exclude_minimal_ports: bool,
	///A vector with link classes whose ports are excluded from the calculations.
	exclude_link_classes: Vec<usize>,
}

impl VirtualChannelPolicy for AverageOccupancyFunction
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy AverageOccupancyFunction");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			let distance = topology.distance(router.get_index().expect("Index should be here"),info.target_router_index);

			candidates.into_iter().map(
				//|CandidateEgress{port,virtual_channel,label,estimated_remaining_hops}|
				|candidate|{
					// let CandidateEgress{port,virtual_channel,label,..} = candidate;
					let CandidateEgress{label,..} = candidate;
					//let minimal_neighbour = topology.neighbour(router.get_index().expect("Index should be here"), port);

					let mut q_avg= 0;
					let mut q_count=0;

					for p_avg in 0..topology.degree(0)  //info.port_available_output_space.expect("port_available_output_space is needed").len() //all the ports
					{
						let (neighbour_location,neighbour_link_class) = topology.neighbour(router.get_index().expect("Index should be here"), p_avg);
						let neighbour_router_index = match neighbour_location
						{
							Location::RouterPort {router_index: neighbour_router, router_port: _neighbour_port} =>
								{
									neighbour_router
								},
							_ =>  panic!(),
						};

						let neighbour_distance = topology.distance(neighbour_router_index,info.target_router_index);

						if (self.exclude_minimal_ports && neighbour_distance < distance) || self.exclude_link_classes.contains(&neighbour_link_class)
						{
							continue;
						}

						q_count+=1;

						q_avg += if self.use_internal_space
						{
							let mut occupied_output_space = 0;
							for i in 0..self.virtual_channels.len()
							{
								let virtual_channel_occupied_output_space=info.virtual_channel_occupied_output_space.expect("virtual_channel_occupied_output_space have not been computed for AverageOccupancyFunction");
								occupied_output_space += virtual_channel_occupied_output_space[p_avg][self.virtual_channels[i]] as i32;
							}

							if self.average_virtual_channels
                            {
								occupied_output_space = occupied_output_space/ self.virtual_channels.len() as i32;
                            }

								occupied_output_space

						}
						else {0} + if self.use_neighbour_space
						{
							let mut occupied_output_space = 0;
							let status=router.get_status_at_emisor(p_avg).expect("This router does not have transmission status");
							for i in 0..self.virtual_channels.len()
							{
								let virtual_channel_occupied_output_space=router.get_maximum_credits_towards(p_avg,self.virtual_channels[i]).expect("we need routers with maximum credits") as i32
									- status.known_available_space_for_virtual_channel(self.virtual_channels[i]).expect("remote available space is not known.") as i32;
								occupied_output_space += virtual_channel_occupied_output_space;
							}

							if self.average_virtual_channels
							{
								occupied_output_space = occupied_output_space/ self.virtual_channels.len() as i32;

							}

								occupied_output_space

						}
						else {0};
					}

					let q = q_avg /q_count;

					let new_label = self.label_coefficient*label + self.occupancy_coefficient*q + self.product_coefficient*label*q + self.constant_coefficient;
					CandidateEgress{label:new_label,..candidate}
				}).collect::<Vec<_>>()
		}
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl AverageOccupancyFunction
{
	pub fn new(arg:VCPolicyBuilderArgument) -> AverageOccupancyFunction
	{
		let mut label_coefficient=None;
		let mut occupancy_coefficient=None;
		let mut product_coefficient=None;
		let mut constant_coefficient=None;
		let mut use_internal_space=false;
		let mut use_neighbour_space=false;
		let mut virtual_channels= None;
		let mut average_virtual_channels=false;
		let mut exclude_minimal_ports=true;
		let mut exclude_link_classes=Vec::new();
		//let mut only_minimal_link_class=false;
		match_object_panic!(arg.cv,"AverageOccupancyFunction",value,
			"label_coefficient" => label_coefficient=Some(value.as_f64().expect("bad value for label_coefficient") as i32),
			"occupancy_coefficient" => occupancy_coefficient=Some(value.as_f64().expect("bad value for occupancy_coefficient") as i32),
			"product_coefficient" => product_coefficient=Some(value.as_f64().expect("bad value for product_coefficient") as i32),
			"constant_coefficient" => constant_coefficient=Some(value.as_f64().expect("bad value for constant_coefficient") as i32),
			"use_neighbour_space" => use_neighbour_space=value.as_bool().expect("bad value for use_neighbour_space"),
			"use_internal_space" => use_internal_space=value.as_bool().expect("bad value for use_internal_space"),
			"exclude_minimal_ports" => exclude_minimal_ports=value.as_bool().expect("bad value for exclude_minimal_ports"),
			"virtual_channels" => virtual_channels=Some(value.as_array().expect("bad value for virtual_channels").iter()
				.map(|v| v.as_f64().expect("bad value for virtual_channels") as usize ).collect::<Vec<_>>()),
			"average_virtual_channels" => average_virtual_channels=value.as_bool().expect("bad value for average_virtual_channels"),
			"exclude_link_classes" => exclude_link_classes=value.as_array().expect("bad value for exclude_link_classes").iter()
                .map(|v| v.as_f64().expect("bad value for exclude_link_classes") as usize ).collect::<Vec<_>>(),
			//"only_minimal_link_class" => only_minimal_link_class=value.as_bool().expect("bad value for only_minimal_link_class"),
		);
		let label_coefficient=label_coefficient.expect("There were no multiplier");
		let occupancy_coefficient=occupancy_coefficient.expect("There were no multiplier");
		let product_coefficient=product_coefficient.expect("There were no multiplier");
		let constant_coefficient=constant_coefficient.expect("There were no multiplier");
		let virtual_channels=virtual_channels.expect("There were no virtual channels");

		AverageOccupancyFunction{
			label_coefficient,
			occupancy_coefficient,
			product_coefficient,
			constant_coefficient,
			use_internal_space,
			use_neighbour_space,
			virtual_channels,
			average_virtual_channels,
			exclude_minimal_ports,
			exclude_link_classes,
		}
	}
}


///Select the egresses with lowest label.
#[derive(Debug)]
pub struct LowestLabel{}

impl VirtualChannelPolicy for LowestLabel
{
	fn filter(&self, candidates:Vec<CandidateEgress>, _router:&dyn Router, _info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		let mut best=vec![];
		let mut best_label=<i32>::MAX;

		//for CandidateEgress{port:p,virtual_channel:vc,label,estimated_remaining_hops} in candidates
		for candidate in candidates
		{
			let label = candidate.label;
			if label<best_label
			{
				best_label=label;
				//best=vec![CandidateEgress{port:p,virtual_channel:vc,label,estimated_remaining_hops}];
				best=vec![candidate];
			}
			else if label==best_label
			{
				//best.push(CandidateEgress{port:p,virtual_channel:vc,label,estimated_remaining_hops});
				best.push(candidate);
			}
		}
		best
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl LowestLabel
{
	pub fn new(_arg:VCPolicyBuilderArgument) -> LowestLabel
	{
		LowestLabel{}
	}
}












///New label = min{old_label,value} or max{old_label,value}
///(value,bottom)
#[derive(Debug)]
pub struct LabelSaturate
{
	value:i32,
	bottom:bool,
}

impl VirtualChannelPolicy for LabelSaturate
{
	fn filter(&self, candidates:Vec<CandidateEgress>, _router:&dyn Router, _info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		if self.bottom
		{
			candidates.into_iter().map(
				//|CandidateEgress{port,virtual_channel,label,estimated_remaining_hops}|
				|candidate|{
				let label= candidate.label;
				//label as usize <= simulation.cycle -1 - self.virtual_ports[port][virtual_channel].last_transmission
				let new_label = std::cmp::max(label,self.value);
				CandidateEgress{label:new_label,..candidate}
			}).collect::<Vec<_>>()
		}
		else
		{
			candidates.into_iter().map(
				|candidate|{
				let label= candidate.label;
				//label as usize <= simulation.cycle -1 - self.virtual_ports[port][virtual_channel].last_transmission
				let new_label = std::cmp::min(label,self.value);
				CandidateEgress{label:new_label,..candidate}
			}).collect::<Vec<_>>()
		}
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl LabelSaturate
{
	pub fn new(arg:VCPolicyBuilderArgument) -> LabelSaturate
	{
		let mut xvalue=None;
		let mut bottom=None;
		match_object_panic!(arg.cv,"LabelSaturate",value,
			"value" => xvalue=Some(value.as_f64().expect("bad value for value") as i32),
			"bottom" => bottom=Some(value.as_bool().expect("bad value for bottom")),
		);
		let value=xvalue.expect("There were no value");
		let bottom=bottom.expect("There were no bottom");
		LabelSaturate{
			value,
			bottom,
		}
	}
}


///New label = old_label*multiplier+summand.
///(multiplier,summand,saturate_bottom,saturate_top,minimum,maximum)
#[derive(Debug)]
pub struct LabelTransform
{
	multiplier:i32,
	summand:i32,
	saturate_bottom: Option<i32>,
	saturate_top: Option<i32>,
	minimum: Option<i32>,
	maximum: Option<i32>,
}

impl VirtualChannelPolicy for LabelTransform
{
	fn filter(&self, candidates:Vec<CandidateEgress>, _router:&dyn Router, _info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		candidates.into_iter().filter_map(
			//|CandidateEgress{port,virtual_channel,label,estimated_remaining_hops}|
			|candidate|{
			let mut new_label = candidate.label*self.multiplier + self.summand;
			//let new_label = ::std::cmp::min(::std::cmp::max(label*self.multiplier + self.summand, saturate_bottom),saturate_top);
			if let Some(value)=self.saturate_bottom
			{
				if value>new_label
				{
					new_label=value;
				}
			}
			if let Some(value)=self.saturate_top
			{
				if value<new_label
				{
					new_label=value;
				}
			}
			//if new_label>=minimum && new_label<=maximum;
			let mut good=true;
			if let Some(value)=self.minimum
			{
				if value>new_label
				{
					good=false;
				}
			}
			if let Some(value)=self.maximum
			{
				if value<new_label
				{
					good=false;
				}
			}
			if good
			{
				Some(CandidateEgress{label:new_label,..candidate})
			}
			else
			{
				None
			}
		}).collect::<Vec<_>>()
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		true
	}

}

impl LabelTransform
{
	pub fn new(arg:VCPolicyBuilderArgument) -> LabelTransform
	{
		let mut multiplier=None;
		let mut summand=None;
		let mut saturate_bottom=None;
		let mut saturate_top=None;
		let mut minimum=None;
		let mut maximum=None;
		match_object_panic!(arg.cv,"LabelTransform",value,
			"multiplier" => multiplier=Some(value.as_f64().expect("bad value for multiplier") as i32),
			"summand" => summand=Some(value.as_f64().expect("bad value for summand") as i32),
			"saturate_bottom" => saturate_bottom=Some(value.as_f64().expect("bad value for saturate_bottom") as i32),
			"saturate_top" => saturate_top=Some(value.as_f64().expect("bad value for saturate_top") as i32),
			"minimum" => minimum=Some(value.as_f64().expect("bad value for minimum") as i32),
			"maximum" => maximum=Some(value.as_f64().expect("bad value for maximum") as i32),
		);
		let multiplier=multiplier.expect("There were no multiplier");
		let summand=summand.expect("There were no summand");
		LabelTransform{
			multiplier,
			summand,
			saturate_bottom,
			saturate_top,
			minimum,
			maximum,
		}
	}
}




///Transform (l,q) into new label a*l+b*q+c*l*q+d
///where l is the label and q is the occupancy.
#[derive(Debug)]
pub struct OccupancyFunction
{
	///Which multiplies the label.
	label_coefficient: i32,
	///Which multiplies the occupancy.
	occupancy_coefficient: i32,
	///Which multiplies the product of label and occupancy.
	product_coefficient: i32,
	///Just added.OccupancyFunction
	constant_coefficient: i32,
	///Whether to include the own router buffers in the calculation.
	use_internal_space: bool,
	///Whether to include the known state of the next router buffers in the calculation.
	use_neighbour_space: bool,
	///Whether to aggregate all virtual channels associated to the port.
	///Defaults to true.
	aggregate: bool,
	///VC to use in the occupancy calculation
	virtual_channels: Option<Vec<usize>>,
}

impl VirtualChannelPolicy for OccupancyFunction
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy OccupancyFunction");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			candidates.into_iter().map(
				//|CandidateEgress{port,virtual_channel,label,estimated_remaining_hops}|
				|candidate|{
				let CandidateEgress{port,virtual_channel,label,..} = candidate;
				let q=if self.use_internal_space
				{
					if self.virtual_channels.is_some()
					{
						let vc = self.virtual_channels.as_ref().expect("Some VCs are indicated");
						let mut occupied_output_space = 0;
						for i in 0..vc.len()
						{
							let virtual_channel_occupied_output_space=info.virtual_channel_occupied_output_space.expect("virtual_channel_occupied_output_space have not been computed for AverageOccupancyFunction");
							occupied_output_space += virtual_channel_occupied_output_space[port][vc[i]] as i32;
						}
						occupied_output_space
					}else if self.aggregate
					{
						let port_occupied_output_space=info.port_occupied_output_space.expect("port_occupied_output_space have not been computed for policy OccupancyFunction");
						port_occupied_output_space[port] as i32
					}
					else
					{
						let virtual_channel_occupied_output_space=info.virtual_channel_occupied_output_space.expect("virtual_channel_occupied_output_space have not been computed for OccupancyFunction");
						virtual_channel_occupied_output_space[port][virtual_channel] as i32
					}
				}
				else {0} + if self.use_neighbour_space
				{
					let status=router.get_status_at_emisor(port).expect("This router does not have transmission status");
					if self.virtual_channels.is_some()
					{
						let vc = self.virtual_channels.as_ref().expect("Some VCs are indicated");
						let mut occupied_next_router = 0;
						for i in 0..vc.len()
						{
							//let virtual_channel_occupied_output_space=info.virtual_channel_occupied_output_space.expect("virtual_channel_occupied_output_space have not been computed for AverageOccupancyFunction");
							let virtual_channels_credits=router.get_maximum_credits_towards(port,vc[i]).expect("we need routers with maximum credits") as i32
								- status.known_available_space_for_virtual_channel(vc[i]).expect("remote available space is not known.") as i32;
							occupied_next_router += virtual_channels_credits;
						}
						occupied_next_router

					}else if self.aggregate
					{
						//port_average_neighbour_queue_length[port]
						//FIXME: this could be different than the whole occuped space if using a DAMQ or something, although they are yet to be implemented.
						(0..status.num_virtual_channels()).map(|c|router.get_maximum_credits_towards(port,c).expect("we need routers with maximum credits") as i32 - status.known_available_space_for_virtual_channel(c).expect("remote available space is not known.") as i32).sum()
					}
					else
					{
						//port_average_neighbour_queue_length[port]
						//let status=router.get_status_at_emisor(port).expect("This router does not have transmission status");
						router.get_maximum_credits_towards(port,virtual_channel).expect("we need routers with maximum credits") as i32 - status.known_available_space_for_virtual_channel(virtual_channel).expect("remote available space is not known.") as i32
					}
				}
				else {0};
				let new_label = self.label_coefficient*label + self.occupancy_coefficient*q + self.product_coefficient*label*q + self.constant_coefficient;
				CandidateEgress{label:new_label,..candidate}
			}).collect::<Vec<_>>()
		}
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl OccupancyFunction
{
	pub fn new(arg:VCPolicyBuilderArgument) -> OccupancyFunction
	{
		let mut label_coefficient=None;
		let mut occupancy_coefficient=None;
		let mut product_coefficient=None;
		let mut constant_coefficient=None;
		let mut use_internal_space=false;
		let mut use_neighbour_space=false;
		let mut aggregate=true;
		let mut virtual_channels=None;
		match_object_panic!(arg.cv,"OccupancyFunction",value,
			"label_coefficient" => label_coefficient=Some(value.as_f64().expect("bad value for label_coefficient") as i32),
			"occupancy_coefficient" => occupancy_coefficient=Some(value.as_f64().expect("bad value for occupancy_coefficient") as i32),
			"product_coefficient" => product_coefficient=Some(value.as_f64().expect("bad value for product_coefficient") as i32),
			"constant_coefficient" => constant_coefficient=Some(value.as_f64().expect("bad value for constant_coefficient") as i32),
			"use_neighbour_space" => use_neighbour_space=value.as_bool().expect("bad value for use_neighbour_space"),
			"use_internal_space" => use_internal_space=value.as_bool().expect("bad value for use_internal_space"),
			"aggregate" => aggregate=value.as_bool().expect("bad value for aggregate"),
			"virtual_channels" => virtual_channels=Some(value.as_array().expect("bad value for virtual channels")
				.iter().map(|a| a.as_usize().expect("It should be a number") ).collect()),
		);
		let label_coefficient=label_coefficient.expect("There were no multiplier");
		let occupancy_coefficient=occupancy_coefficient.expect("There were no multiplier");
		let product_coefficient=product_coefficient.expect("There were no multiplier");
		let constant_coefficient=constant_coefficient.expect("There were no multiplier");

		OccupancyFunction{
			label_coefficient,
			occupancy_coefficient,
			product_coefficient,
			constant_coefficient,
			use_internal_space,
			use_neighbour_space,
			aggregate,
			virtual_channels,
		}
	}
}


///New label = -old_label
///Just until I fix the grammar to accept preceding minuses.
#[derive(Debug)]
pub struct NegateLabel
{
}

impl VirtualChannelPolicy for NegateLabel
{
	fn filter(&self, candidates:Vec<CandidateEgress>, _router:&dyn Router, _info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		candidates.into_iter().map(
			//|CandidateEgress{port,virtual_channel,label,estimated_remaining_hops}|
			|candidate|CandidateEgress{label:-candidate.label,..candidate}
		).collect::<Vec<_>>()
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl NegateLabel
{
	pub fn new(arg:VCPolicyBuilderArgument) -> NegateLabel
	{
		match_object_panic!(arg.cv,"NegateLabel",_value);
		NegateLabel{}
	}
}




///Vector of labels
///`new_label = vector[old_label]`
#[derive(Debug)]
pub struct VecLabel
{
	label_vector: Vec<i32>,
}

impl VirtualChannelPolicy for VecLabel
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy VecLabel");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			candidates.into_iter().map(
				//|CandidateEgress{port,virtual_channel,label,estimated_remaining_hops}|
				|candidate|{
				let label = candidate.label;
				if label<0 || label>=self.label_vector.len() as i32
				{
					panic!("label={} is out of range 0..{}",label,self.label_vector.len());
				}
				let new_label = self.label_vector[label as usize];
				CandidateEgress{label:new_label,..candidate}
			}).collect::<Vec<_>>()
		}
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl VecLabel
{
	pub fn new(arg:VCPolicyBuilderArgument) -> VecLabel
	{
		let mut label_vector=None;
		match_object_panic!(arg.cv,"VecLabel",value,
			"label_vector" => label_vector=Some(value.as_array().expect("bad value for label_vector").iter()
				.map(|v|v.as_f64().expect("bad value in label_vector") as i32).collect()),
		);
		let label_vector=label_vector.expect("There were no label_vector");
		VecLabel{
			label_vector,
		}
	}
}

///Apply a different policy to candidates with each label.
#[derive(Debug)]
pub struct MapLabel
{
	label_to_policy: Vec<Box<dyn VirtualChannelPolicy>>,
	below_policy: Box<dyn VirtualChannelPolicy>,
	above_policy: Box<dyn VirtualChannelPolicy>,
}

impl VirtualChannelPolicy for MapLabel
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, topology:&dyn Topology, rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy MapLabel");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			let n = self.label_to_policy.len();
			//above goes into candidate_map[ labels  ]
			//below goes into candidate_map[ labels+1  ]
			let mut candidate_map = vec![vec![];n+2];
			for cand in candidates.into_iter()
			{
				let label : usize = if cand.label < 0
				{
					self.label_to_policy.len()+1
				} else if cand.label > n.try_into().unwrap() {
					n
				} else {
					cand.label.try_into().unwrap()
				};
				candidate_map[label].push(cand);
			}
			let mut policies = self.label_to_policy.iter().chain( vec![&self.above_policy].into_iter() ).chain( vec![&self.below_policy].into_iter() );
			let mut r = vec![];
			for candidate_list in candidate_map
			{
				let policy : &dyn VirtualChannelPolicy = policies.next().unwrap().as_ref();
				r.extend( policy.filter(candidate_list,router,info,topology,rng)  );
			}
			r
		}
	}

	fn need_server_ports(&self)->bool
	{
		true
	}

	fn need_port_average_queue_length(&self)->bool
	{
		true
	}

	fn need_port_last_transmission(&self)->bool
	{
		true
	}

}

impl MapLabel
{
	pub fn new(arg:VCPolicyBuilderArgument) -> MapLabel
	{
		let mut label_to_policy=None;
		let mut below_policy : Box<dyn VirtualChannelPolicy> =Box::new(Identity{});
		let mut above_policy : Box<dyn VirtualChannelPolicy> =Box::new(Identity{});
		match_object_panic!(arg.cv,"MapLabel",value,
			"label_to_policy" => label_to_policy=Some(value.as_array().expect("bad value for label_to_policy").iter()
				.map(|v|new_virtual_channel_policy(VCPolicyBuilderArgument{cv:v,..arg})).collect()),
			"below_policy" => below_policy = new_virtual_channel_policy(VCPolicyBuilderArgument{cv:value,..arg}),
			"above_policy" => above_policy = new_virtual_channel_policy(VCPolicyBuilderArgument{cv:value,..arg}),
		);
		let label_to_policy=label_to_policy.expect("There were no label_to_policy");
		MapLabel{
			label_to_policy,
			below_policy,
			above_policy,
		}
	}
}


///Only allows those candidates whose vc equals their entry vc plus some `s` in `shifts`.
#[derive(Debug)]
pub struct ShiftEntryVC
{
	shifts: Vec<i32>,
}

impl VirtualChannelPolicy for ShiftEntryVC
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy ShiftEntryVC");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			let evc = info.entry_virtual_channel as i32;
			candidates.into_iter().filter(|&CandidateEgress{virtual_channel,..}| self.shifts.contains(&(virtual_channel as i32-evc)) ).collect::<Vec<_>>()
		}
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl ShiftEntryVC
{
	pub fn new(arg:VCPolicyBuilderArgument) -> ShiftEntryVC
	{
		let mut shifts=None;
		match_object_panic!(arg.cv,"ShiftEntryVC",value,
			"shifts" => shifts=Some(value.as_array().expect("bad value for shifts").iter()
				.map(|v|v.as_f64().expect("bad value in shifts") as i32).collect()),
		);
		let shifts=shifts.expect("There were no shifts");
		ShiftEntryVC{
			shifts,
		}
	}
}


///Apply a different policy to each hop.
#[derive(Debug)]
pub struct MapHop
{
	hop_to_policy: Vec<Box<dyn VirtualChannelPolicy>>,
	above_policy: Box<dyn VirtualChannelPolicy>,
}

impl VirtualChannelPolicy for MapHop
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, topology:&dyn Topology, rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy MapHop");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			let policy = if info.performed_hops>=self.hop_to_policy.len() { &self.above_policy } else { &self.hop_to_policy[info.performed_hops] };
			policy.filter(candidates,router,info,topology,rng)
		}
	}

	fn need_server_ports(&self)->bool
	{
		true
	}

	fn need_port_average_queue_length(&self)->bool
	{
		true
	}

	fn need_port_last_transmission(&self)->bool
	{
		true
	}

}

impl MapHop
{
	pub fn new(arg:VCPolicyBuilderArgument) -> MapHop
	{
		let mut hop_to_policy=None;
		let mut above_policy : Box<dyn VirtualChannelPolicy> =Box::new(Identity{});
		match_object_panic!(arg.cv,"MapHop",value,
			"hop_to_policy" => hop_to_policy=Some(value.as_array().expect("bad value for hop_to_policy").iter()
				.map(|v|new_virtual_channel_policy(VCPolicyBuilderArgument{cv:v,..arg})).collect()),
			"above_policy" => above_policy = new_virtual_channel_policy(VCPolicyBuilderArgument{cv:value,..arg}),
		);
		let hop_to_policy=hop_to_policy.expect("There were no hop_to_policy");
		MapHop{
			hop_to_policy,
			above_policy,
		}
	}
}

///Only allows those candidates whose vc is in the allowed list.
#[derive(Debug)]
pub struct ArgumentVC
{
	allowed: Vec<usize>,
}

impl VirtualChannelPolicy for ArgumentVC
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, _topology:&dyn Topology, _rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy ArgumentVC");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			candidates.into_iter().filter(|&CandidateEgress{virtual_channel,..}| self.allowed.contains(&virtual_channel) ).collect::<Vec<_>>()
		}
	}

	fn need_server_ports(&self)->bool
	{
		false
	}

	fn need_port_average_queue_length(&self)->bool
	{
		false
	}

	fn need_port_last_transmission(&self)->bool
	{
		false
	}

}

impl ArgumentVC
{
	pub fn new(arg:VCPolicyBuilderArgument) -> ArgumentVC
	{
		let mut allowed=None;
		match_object_panic!(arg.cv,"ArgumentVC",value,
			"allowed" => allowed=Some(value.as_array().expect("bad value for allowed").iter()
				.map(|v|v.as_f64().expect("bad value in allowed") as usize).collect()),
		);
		let allowed=allowed.expect("There were no allowed");
		ArgumentVC{
			allowed,
		}
	}
}

///Accepts with any of the policies given.
#[derive(Debug)]
pub struct Either
{
	policies: Vec<Box<dyn VirtualChannelPolicy>>,
}

impl VirtualChannelPolicy for Either
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, topology:&dyn Topology, rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy Either");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			let mut r = Vec::new();
			for policy in self.policies.iter()
			{
				r.extend(policy.as_ref().filter(candidates.clone(),router,info,topology,rng));
			}
			r
		}
	}

	fn need_server_ports(&self)->bool
	{
		true
	}

	fn need_port_average_queue_length(&self)->bool
	{
		true
	}

	fn need_port_last_transmission(&self)->bool
	{
		true
	}

}

impl Either
{
	pub fn new(arg:VCPolicyBuilderArgument) -> Either
	{
		let mut policies=None;
		match_object_panic!(arg.cv,"Either",value,
			"policies" => policies=Some(value.as_array().expect("bad value for policies").iter()
				.map(|v|new_virtual_channel_policy(VCPolicyBuilderArgument{cv:v,..arg})).collect()),
		);
		let policies=policies.expect("There were no policies");
		Either{
			policies,
		}
	}
}

///Apply a different policy to candidates from each entry virtual channel.
#[derive(Debug)]
pub struct MapEntryVC
{
	///Which policy to apply, index by the entry virtual channel.
	vc_to_policy: Vec<Box<dyn VirtualChannelPolicy>>,
	///Policy to apply if entry virtual channel is above the array range limit.
	above_policy: Box<dyn VirtualChannelPolicy>,
}

impl VirtualChannelPolicy for MapEntryVC
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, topology:&dyn Topology, rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy MapEntryVC");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			let n = self.vc_to_policy.len();
			let evc = info.entry_virtual_channel;
			let policy = if evc<n
			{
				&self.vc_to_policy[evc]
			} else {
				&self.above_policy
			};
			policy.filter(candidates,router,info,topology,rng)
		}
	}

	fn need_server_ports(&self)->bool
	{
		true
	}

	fn need_port_average_queue_length(&self)->bool
	{
		true
	}

	fn need_port_last_transmission(&self)->bool
	{
		true
	}

}

impl MapEntryVC
{
	pub fn new(arg:VCPolicyBuilderArgument) -> MapEntryVC
	{
		let mut vc_to_policy=None;
		let mut above_policy : Box<dyn VirtualChannelPolicy> =Box::new(Identity{});
		match_object_panic!(arg.cv,"MapEntryVC",value,
			"vc_to_policy" => vc_to_policy=Some(value.as_array().expect("bad value for vc_to_policy").iter()
				.map(|v|new_virtual_channel_policy(VCPolicyBuilderArgument{cv:v,..arg})).collect()),
			"above_policy" => above_policy = new_virtual_channel_policy(VCPolicyBuilderArgument{cv:value,..arg}),
		);
		let vc_to_policy=vc_to_policy.expect("There were no vc_to_policy");
		MapEntryVC{
			vc_to_policy,
			above_policy,
		}
	}
}


///Apply a different policy to candidates whose messages have their size in different ranges.
///For example, with `limits:[160]` and `policies:[Identity,ArgumentVC{allowed:[2,3]}]` we force packets in long messages to use some given virtual channels.
#[derive(Debug)]
pub struct MapMessageSize
{
	///Which policy to apply, index by the range in which they are included.
	///`policy` must have exactly one element more than `limits`.
	policies: Vec<Box<dyn VirtualChannelPolicy>>,
	///Exclusive superior limits of the ranges. There is another one which is unbounded.
	limits: Vec<usize>,
}

impl VirtualChannelPolicy for MapMessageSize
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, topology:&dyn Topology, rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			let message_size = info.phit.packet.message.size;
			let mut index = 0usize;
			while index<self.limits.len() && message_size >= self.limits[index]
			{
				index+=1;
			}
			self.policies[index].filter(candidates,router,info,topology,rng)
		}
	}

	fn need_server_ports(&self)->bool
	{
		true
	}

	fn need_port_average_queue_length(&self)->bool
	{
		true
	}

	fn need_port_last_transmission(&self)->bool
	{
		true
	}

}

impl MapMessageSize
{
	pub fn new(arg:VCPolicyBuilderArgument) -> MapMessageSize
	{
		let mut policies : Option<Vec<_>> =None;
		let mut limits : Option<Vec<_>> =None;
		match_object_panic!(arg.cv,"MapMessageSize",value,
			"policies" => policies=Some(value.as_array().expect("bad value for policies").iter()
				.map(|v|new_virtual_channel_policy(VCPolicyBuilderArgument{cv:v,..arg})).collect()),
			"limits" => limits=Some(value.as_array().expect("bad value for limits").iter()
				.map(|v|v.as_f64().expect("bad value in limits") as usize).collect()),
		);
		let policies=policies.expect("There were no policies");
		let limits=limits.expect("There were no limits");
		assert_eq!(policies.len(), limits.len() + 1, "In MapMessageSize the `policies` array must have one element more than `limits`, as the last range is unbounded.");
		MapMessageSize{
			policies,
			limits,
		}
	}
}


/// Accepts if the sequence of policies accept. Empty is a NOP. Just for meta-policies.
#[derive(Debug)]
pub struct Chain
{
	policies: Vec<Box<dyn VirtualChannelPolicy>>,
}

impl VirtualChannelPolicy for Chain
{
	fn filter(&self, mut candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, topology:&dyn Topology, rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy Chain");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			for policy in self.policies.iter()
			{
				candidates = policy.as_ref().filter(candidates,router,info,topology,rng);
			}
			candidates
		}
	}

	fn need_server_ports(&self)->bool
	{
		true
	}

	fn need_port_average_queue_length(&self)->bool
	{
		true
	}

	fn need_port_last_transmission(&self)->bool
	{
		true
	}
}

impl Chain
{
	pub fn new(arg:VCPolicyBuilderArgument) -> Chain
	{
		let mut policies=None;
		match_object_panic!(arg.cv,"Chain",value,
			"policies" => policies=Some(value.as_array().expect("bad value for policies").iter()
				.map(|v|new_virtual_channel_policy(VCPolicyBuilderArgument{cv:v,..arg})).collect()),
		);
		let policies=policies.expect("There were no policies");
		Chain{
			policies,
		}
	}
}


/**
Employ a different VC (or policy) to each destination.

Example configuration:
```ignore
VOQ{
	/// Optionally set a number of VCs to use in this policy. By default it uses a VC per destination node.
	/// Packets to destination `dest` will use VC number `(dest % num_classes) + start_virtual_channel`.
	//num_classes: 4,
	/// Optionally, use the index of the destination switch instead of the destination server.
	switch_level: true,
	/// Optionally, give specific policies for matching indices instead of just just such index as VC.
	/// If this example had `num_classes=2`, then it would use the Identity policy for even destinations and the Hops policy for odd destinations.
	/// It can be though as having a default of infinite array full of ArgumentVC whose argument equal to the array index.
	// policies_override: [Identity,Hops],
}
```
**/
#[derive(Debug)]
pub struct VOQ
{
	/// Optionally set a number of VCs to use in this policy. By default it uses a VC per destination node.
	/// Packets to destination `dest` will use VC number `(dest % num_classes) + start_virtual_channel`.
	num_classes: Option<usize>,
	/// Whether to index by target switch instead of target server.
	switch_level: bool,
	/// The channel to be use for the destination 0.
	start_virtual_channel: usize,
	/// Whether to use use a specific policy for matching indices instead of just just such index as VC.
	/// For example with `num_classes=2` it will use one policy for even destinations and other for odd destinations.
	policies_override: Vec<Box<dyn VirtualChannelPolicy>>,
}

impl VirtualChannelPolicy for VOQ
{
	fn filter(&self, candidates:Vec<CandidateEgress>, router:&dyn Router, info: &RequestInfo, topology:&dyn Topology, rng: &mut StdRng) -> Vec<CandidateEgress>
	{
		//let port_average_neighbour_queue_length=port_average_neighbour_queue_length.as_ref().expect("port_average_neighbour_queue_length have not been computed for policy VOQ");
		if router.get_index().expect("we need routers with index") == info.target_router_index
		{
			//do nothing
			candidates
		}
		else
		{
			let destination = if self.switch_level { info.target_router_index  } else { info.phit.packet.message.destination };
			let index = match self.num_classes
			{
				None => destination,
				Some(n) => destination % n,
			};
			if index < self.policies_override.len() {
				self.policies_override[index].filter(candidates,router,info,topology,rng)
			} else {
				let vc = index + self.start_virtual_channel;
				candidates.into_iter().filter(
					|&CandidateEgress{port:_,virtual_channel,label:_,estimated_remaining_hops:_,..}| vc==virtual_channel
				).collect::<Vec<_>>()
			}
		}
	}

	fn need_server_ports(&self)->bool
	{
		true
	}

	fn need_port_average_queue_length(&self)->bool
	{
		true
	}

	fn need_port_last_transmission(&self)->bool
	{
		true
	}
}

impl VOQ
{
	pub fn new(arg:VCPolicyBuilderArgument) -> VOQ
	{
		let mut num_classes = None;
		let mut switch_level = false;
		let mut start_virtual_channel = 0;
		let mut policies_override=vec![];
		match_object_panic!(arg.cv,"VOQ",value,
			"num_classes" => num_classes = Some(value.as_usize().expect("bad value for num_classes")),
			"switch_level" => switch_level = value.as_bool().expect("bad value for switch_level"),
			"start_virtual_channel" => start_virtual_channel = value.as_usize().expect("bad value for start_virtual_channel"),
			"policies_override" => policies_override=value.as_array().expect("bad value for policies_override").iter()
				.map(|v|new_virtual_channel_policy(VCPolicyBuilderArgument{cv:v,..arg})).collect(),
		);
		VOQ{
			num_classes,
			switch_level,
			start_virtual_channel,
			policies_override,
		}
	}
}








