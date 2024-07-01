
use std::cell::RefCell;
use std::rc::{Rc,Weak};
use std::ops::{Deref};
use std::mem::{size_of};
use ::rand::{Rng,rngs::StdRng,prelude::SliceRandom};
use std::convert::TryInto;

use super::{Router,AbstractTransmissionMechanism,StatusAtEmissor,SpaceAtReceptor,AugmentedBuffer,AcknowledgeMessage,RouterBuilderArgument,new_transmission_mechanism,TransmissionMechanismBuilderArgument};
use crate::config_parser::ConfigurationValue;
use crate::topology::{Location,Topology};
use crate::routing::CandidateEgress;
use crate::policies::{RequestInfo,VirtualChannelPolicy,new_virtual_channel_policy,VCPolicyBuilderArgument};
use crate::event::{Event,Eventful,EventGeneration,CyclePosition,Time};
use crate::{Phit,Packet,SimulationShared,SimulationMut};
use crate::quantify::Quantifiable;
use crate::packet::PacketRef;
//use crate::Plugs;
use crate::match_object_panic;


///Strategy for the arbitration of the output port.
enum OutputArbiter
{
	#[allow(dead_code)]
	Random,
	Token{
		port_token: Vec<usize>,
	},
}

///The basic Router struct. Very similar to FSIN's router.
pub struct Basic
{
	///Weak pointer to itself, see <https://users.rust-lang.org/t/making-a-rc-refcell-trait2-from-rc-refcell-trait1/16086/3>
	self_rc: Weak<RefCell<Basic>>,
	///When is the next scheduled event. Stack with the soonner event the last.
	next_events: Vec<Time>,
	///The cycle number of the last time Basic::process was called. Only for debugging/assertion purposes.
	last_process_at_cycle: Option<Time>,
	///Its index in the topology
	router_index: usize,
	///The mechanism to select virtual channels
	virtual_channel_policies: Vec<Box<dyn VirtualChannelPolicy>>,
	///If the bubble mechanism is active
	bubble: bool,
	///Credits required in the next router's virtual port to begin the transmission
	flit_size: usize,
	///Size of each input buffer.
	buffer_size: usize,
	///Give priority to in-transit packets over packets in injection queues.
	intransit_priority: bool,
	///To allow to request a port even if some other packet is being transmitted throught it to a different virtual channel (as FSIN does).
	///It may appear that should obviously be put to `true`, but in practice that just reduces performance.
	allow_request_busy_port: bool,
	///Use the labels provided by the routing to sort the petitions in the output arbiter.
	output_prioritize_lowest_label: bool,
	///Whether to immediately discard candidate outputs when they are currently receiving from an input.
	///Otherwise, these candidates are marked as impossible, but they can be processed by the `virtual_channel_policies`.
	///In particular, [EnforceFlowControl] will filter them out.
	///Defaults to false.
	neglect_busy_output: bool,
	/// `transmission_port_status[port] = status`
	transmission_port_status: Vec<Box<dyn StatusAtEmissor>>,
	/// `reception_port_space[port] = space`
	reception_port_space: Vec<Box<dyn SpaceAtReceptor>>,
	// The router to router mechanism employed.
	// transmission_mechanism: Box<dyn AbstractTransmissionMechanism>,
	// The router to server mechanism employed.
	// to_server_mechanism: Box<dyn AbstractTransmissionMechanism>,
	/// The server to router mechanism employed.
	/// This will be used to build the status at the servers.
	from_server_mechanism: Box<dyn AbstractTransmissionMechanism>,
	///If 0 then there are no output buffer, if greater than 0 then the size of each of them.
	output_buffer_size: usize,
	///The outut buffers indexed as `[output_port][output_vc]`.
	///Phits are stored with their `(entry_port,entry_vc)`.
	output_buffers: Vec<Vec<AugmentedBuffer<(usize,usize)>>>,
	///If not None then the input port+virtual_channel which is either sending by this port+virtual_channel or writing to this output buffer.
	///We keep the packet for debugging/check considerations.
	selected_input: Vec<Vec<Option<(PacketRef,usize,usize)>>>,
	///If not None then all the phits should go through this port+virtual_channel or stored in this output buffer, since they are part of the same packet
	///We keep the packet for debugging/check considerations.
	selected_output: Vec<Vec<Option<(PacketRef,usize,usize)>>>,
	///Number of cycles that the current phit, if any, in the head of a given (port,virtual channel) input buffer the phit has been waiting.
	time_at_input_head: Vec<Vec<usize>>,
	///And arbiter of the physical output port.
	output_arbiter: OutputArbiter,
	///The maximum packet size that is allowed. Only for bubble consideration, that reserves space for a given packet plus maximum packet size.
	maximum_packet_size: usize,

	//statistics:
	statistics_temporal_step: Time,
	principal_measurement: BasicRouterMeasurement,
	temporal_statistics: Vec<BasicRouterMeasurement>,
}

#[derive(Clone)]
pub struct BasicRouterMeasurement
{
	///The first cycle included in the statistics.
	pub begin_cycle: Time,
	///Accumulated over time, averaged per port.
	pub output_buffer_occupation_per_vc: Vec<f64>,
	///Accumulated over time, averaged per port.
	pub reception_space_occupation_per_vc: Vec<f64>,
}

impl BasicRouterMeasurement
{
	/// The number of virtual_channels in use
	pub fn new(virtual_channels: usize) -> BasicRouterMeasurement
	{
		BasicRouterMeasurement{
			begin_cycle: 0,
			output_buffer_occupation_per_vc: vec![0f64;virtual_channels],
			reception_space_occupation_per_vc: vec![0f64;virtual_channels],
		}
	}
	pub fn into_mul(mut self, factor:f64 ) -> BasicRouterMeasurement
	{
		self.mul(factor);
		self
	}
	pub fn mul(&mut self, factor:f64 )->&mut BasicRouterMeasurement
	{
		for elem in self.output_buffer_occupation_per_vc.iter_mut()
		{
			*elem *= factor;
		}
		for elem in self.reception_space_occupation_per_vc.iter_mut()
		{
			*elem *= factor;
		}
		self
	}
}

impl Router for Basic
{
	fn insert(&mut self, current_cycle:Time, phit:Rc<Phit>, port:usize, rng: &mut StdRng) -> Vec<EventGeneration>
	{
		self.reception_port_space[port].insert(phit,rng).expect("there was some problem on the insertion");
		if let Some(event) = self.schedule(current_cycle,0) {
			vec![event]
		} else {
			vec![]
		}
	}
	fn acknowledge(&mut self, current_cycle:Time, port:usize, ack_message:AcknowledgeMessage) -> Vec<EventGeneration>
	{
		self.transmission_port_status[port].acknowledge(ack_message);
		if let Some(event) = self.schedule(current_cycle,0) {
			vec![event]
		} else {
			vec![]
		}
	}
	fn num_virtual_channels(&self) -> usize
	{
		//self.virtual_ports[0].len()
		self.transmission_port_status[0].num_virtual_channels()
	}
	fn virtual_port_size(&self, _port:usize, _virtual_channel:usize) -> usize
	{
		self.buffer_size
	}
	fn iter_phits(&self) -> Box<dyn Iterator<Item=Rc<Phit>>>
	{
		//unimplemented!();
		//Box::new(self.virtual_ports.iter().flat_map(|port|port.iter().flat_map(|vp|vp.iter_phits())).collect::<Vec<_>>().into_iter())
		Box::new(self.reception_port_space.iter().flat_map(|space|space.iter_phits()).collect::<Vec<_>>().into_iter())
	}
	//fn get_virtual_port(&self, port:usize, virtual_channel:usize) -> Option<&VirtualPort>
	//{
	//	Some(&self.virtual_ports[port][virtual_channel])
	//}
	fn get_status_at_emisor(&self, port:usize) -> Option<&dyn StatusAtEmissor>
	{
		Some(&*self.transmission_port_status[port])
	}
	fn get_maximum_credits_towards(&self, _port:usize, _virtual_channel:usize) -> Option<usize>
	{
		Some(self.buffer_size)
	}
	fn get_index(&self)->Option<usize>
	{
		Some(self.router_index)
	}
	fn aggregate_statistics(&self, statistics:Option<ConfigurationValue>, router_index:usize, total_routers:usize, cycle:Time) -> Option<ConfigurationValue>
	{
		//let n_ports = self.selected_input.len();
		//let n_vcs = self.selected_input[0].len();
		//let mut output_buffer_occupation_per_vc:Option<Vec<f64>>= if self.output_buffer_size==0 {None} else
		//{
		//	Some((0..n_vcs).map(|vc|self.output_buffers.iter().map(|port|port[vc].len()).sum::<usize>() as f64).collect())
		//};
		let cycle_span = cycle - self.principal_measurement.begin_cycle;
		let mut reception_space_occupation_per_vc:Option<Vec<f64>> = Some(self.principal_measurement.reception_space_occupation_per_vc.iter().map(|x|x/cycle_span as f64).collect());
		let mut output_buffer_occupation_per_vc:Option<Vec<f64>> = Some(self.principal_measurement.output_buffer_occupation_per_vc.iter().map(|x|x/cycle_span as f64).collect());
		let mut temporal_statistics: Vec<  BasicRouterMeasurement  > = self.temporal_statistics.iter().map(|m|m.clone().into_mul(1f64/self.statistics_temporal_step as f64)).collect();
		if let Some(previous)=statistics
		{
			if let ConfigurationValue::Object(cv_name,previous_pairs) = previous
			{
				if cv_name!="Basic"
				{
					panic!("incompatible statistics, should be `Basic` object not `{}`",cv_name);
				}
				for (ref name,ref value) in previous_pairs
				{
					match name.as_ref()
					{
						"average_output_buffer_occupation_per_vc" => match value
						{
							&ConfigurationValue::Array(ref prev_a) =>
							{
								if let Some(ref mut curr_a) = output_buffer_occupation_per_vc
								{
									for (c,p) in curr_a.iter_mut().zip(prev_a.iter())
									{
										if let ConfigurationValue::Number(x)=p
										{
											*c += x;
										}
										else
										{
											panic!("The non-number {:?} cannot be added",p);
										}
									}
								}
								else
								{
									println!("Ignoring average_output_buffer_occupation_per_vc.");
								}
							}
							_ => panic!("bad value for average_output_buffer_occupation_per_vc"),
						},
						"average_reception_space_occupation_per_vc" => match value
						{
							&ConfigurationValue::Array(ref prev_a) =>
							{
								if let Some(ref mut curr_a) = reception_space_occupation_per_vc
								{
									for (c,p) in curr_a.iter_mut().zip(prev_a.iter())
									{
										if let ConfigurationValue::Number(x)=p
										{
											*c += x;
										}
										else
										{
											panic!("The non-number {:?} cannot be added",p);
										}
									}
								}
								else
								{
									println!("Ignoring average_reception_space_occupation_per_vc.");
								}
							}
							_ => panic!("bad value for average_reception_space_occupation_per_vc"),
						},
						"temporal_statistics" => match value
						{
							&ConfigurationValue::Object(_, ref prev_t_pairs) =>
							{
								let mut local_average_output_buffer_occupation_per_vc : Vec<Vec<f64>> = vec![];
								let mut local_average_reception_space_occupation_per_vc : Vec<Vec<f64>> = vec![];
								for (ref t_name,ref t_value) in prev_t_pairs
								{
									match t_name.as_ref()
									{
										"average_output_buffer_occupation_per_vc" => match t_value
										{
											&ConfigurationValue::Array(ref prev_a) =>
											{
												local_average_output_buffer_occupation_per_vc = prev_a.iter().map(|outer|match outer{
													&ConfigurationValue::Array(ref outer_a) => outer_a.iter().map(|inner|match inner{
														&ConfigurationValue::Number(ref inner_x) => *inner_x,
														_ => panic!(),
													}).collect(),
													_ => panic!(),
												}).collect();
											}
											_ => panic!("bad value for average_output_buffer_occupation_per_vc"),
										},
										"average_reception_space_occupation_per_vc" => match t_value
										{
											&ConfigurationValue::Array(ref prev_a) =>
											{
												local_average_reception_space_occupation_per_vc = prev_a.iter().map(|outer|match outer{
													&ConfigurationValue::Array(ref outer_a) => outer_a.iter().map(|inner|match inner{
														&ConfigurationValue::Number(ref inner_x) => *inner_x,
														_ => panic!(),
													}).collect(),
													_ => panic!(),
												}).collect();
											}
											_ => panic!("bad value for average_reception_space_occupation_per_vc"),
										},
										_ => panic!(),
									}
								}
								let n = temporal_statistics.len()
									.max(local_average_output_buffer_occupation_per_vc.len())
									.max(local_average_reception_space_occupation_per_vc.len());
								//assert!(n==local_average_output_buffer_occupation_per_vc.len());
								//assert!(n==local_average_reception_space_occupation_per_vc.len());
								let vcs = self.num_virtual_channels();
								temporal_statistics.resize_with(n,||{
									// we can safely ignore `begin_cycle` as is only used when gathering data.
									BasicRouterMeasurement::new(vcs)
								});
								//println!("temporal_statistics.len()={}",temporal_statistics.len());
								for temporal_index in 0..temporal_statistics.len()
								{
									for measurement_index in 0..temporal_statistics[temporal_index].output_buffer_occupation_per_vc.len()
									{
										if temporal_index<local_average_output_buffer_occupation_per_vc.len()
										{
											temporal_statistics[temporal_index].output_buffer_occupation_per_vc[measurement_index] += local_average_output_buffer_occupation_per_vc[temporal_index][measurement_index];
										}
										if temporal_index<local_average_reception_space_occupation_per_vc.len()
										{
											temporal_statistics[temporal_index].reception_space_occupation_per_vc[measurement_index] += local_average_reception_space_occupation_per_vc[temporal_index][measurement_index];
										}
									}
								}
							}
							_ => panic!("bad value for temporal_statistics"),
						},
						_ => panic!("Nothing to do with field {} in Basic statistics",name),
					}
				}
			}
			else
			{
				panic!("received incompatible statistics");
			}
		}
		let mut result_content : Vec<(String,ConfigurationValue)> = vec![
			//(String::from("injected_load"),ConfigurationValue::Number(injected_load)),
			//(String::from("accepted_load"),ConfigurationValue::Number(accepted_load)),
			//(String::from("average_message_delay"),ConfigurationValue::Number(average_message_delay)),
			//(String::from("server_generation_jain_index"),ConfigurationValue::Number(jsgp)),
			//(String::from("server_consumption_jain_index"),ConfigurationValue::Number(jscp)),
			//(String::from("average_packet_hops"),ConfigurationValue::Number(average_packet_hops)),
			//(String::from("total_packet_per_hop_count"),ConfigurationValue::Array(total_packet_per_hop_count)),
			//(String::from("average_link_utilization"),ConfigurationValue::Number(average_link_utilization)),
			//(String::from("maximum_link_utilization"),ConfigurationValue::Number(maximum_link_utilization)),
			//(String::from("git_id"),ConfigurationValue::Literal(format!("\"{}\"",git_id))),
		];
		let is_last = router_index+1==total_routers;
		if let Some(ref mut content)=output_buffer_occupation_per_vc
		{
			if is_last
			{
				let factor=1f64 / total_routers as f64;
				for x in content.iter_mut()
				{
					*x *= factor;
				}
			}
			result_content.push((String::from("average_output_buffer_occupation_per_vc"),ConfigurationValue::Array(content.iter().map(|x|ConfigurationValue::Number(*x)).collect())));
		}
		if let Some(ref mut content)=reception_space_occupation_per_vc
		{
			if is_last
			{
				let factor=1f64 / total_routers as f64;
				for x in content.iter_mut()
				{
					*x *= factor;
				}
			}
			result_content.push((String::from("average_reception_space_occupation_per_vc"),ConfigurationValue::Array(content.iter().map(|x|ConfigurationValue::Number(*x)).collect())));
		}
		if !temporal_statistics.is_empty()
		{
			if is_last
			{
				let factor=1f64 / total_routers as f64;
				for m in temporal_statistics.iter_mut()
				{
					m.mul(factor);
				}
			}
			let temporal_content = vec![
				(String::from("average_output_buffer_occupation_per_vc"),ConfigurationValue::Array(temporal_statistics.iter().map(|m|ConfigurationValue::Array(m.output_buffer_occupation_per_vc.iter().map(|x|ConfigurationValue::Number(*x)).collect())).collect())),
				(String::from("average_reception_space_occupation_per_vc"),ConfigurationValue::Array(temporal_statistics.iter().map(|m|ConfigurationValue::Array(m.reception_space_occupation_per_vc.iter().map(|x|ConfigurationValue::Number(*x)).collect())).collect())),
			];
			result_content.push((String::from("temporal_statistics"),ConfigurationValue::Object(String::from("TemporalStatistics"),temporal_content)));
		}
		Some(ConfigurationValue::Object(String::from("Basic"),result_content))
	}
	fn reset_statistics(&mut self, next_cycle:Time)
	{
		self.principal_measurement.begin_cycle=next_cycle;
		for x in self.principal_measurement.output_buffer_occupation_per_vc.iter_mut()
		{
			*x=0f64;
		}
		for x in self.principal_measurement.reception_space_occupation_per_vc.iter_mut()
		{
			*x=0f64;
		}
	}
	fn build_emissor_status(&self, port:usize, topology:&dyn Topology) -> Box<dyn StatusAtEmissor+'static>
	{
		if let (Location::ServerPort(_server),_link_class)=topology.neighbour(self.router_index,port)
		{
			//let from_server_mechanism = TransmissionFromServer::new(self.num_virtual_channels(),self.buffer_size,self.flit_size);
			//Box::new(from_server_mechanism.new_status_at_emissor())
			self.from_server_mechanism.new_status_at_emissor()
		}
		else
		{
			unimplemented!()
		}
	}
}

impl Basic
{
	//pub fn new(router_index: usize, cv:&ConfigurationValue, plugs:&Plugs, topology:&dyn Topology, maximum_packet_size:usize) -> Rc<RefCell<Basic<SimpleVirtualChannels>>>
	pub fn new(arg:RouterBuilderArgument) -> Rc<RefCell<Basic>>
	{
		let RouterBuilderArgument{
			router_index,
			cv,
			plugs,
			topology,
			maximum_packet_size,
			statistics_temporal_step,
			..
		} = arg;
		//let mut servers=None;
		//let mut load=None;
		let mut virtual_channels=None;
		//let mut routing=None;
		let mut buffer_size=None;
		let mut virtual_channel_policies=None;
		let mut bubble=None;
		let mut flit_size=None;
		let mut intransit_priority=None;
		let mut allow_request_busy_port=None;
		let mut output_prioritize_lowest_label=None;
		let mut output_buffer_size=None;
		let mut transmission_mechanism=None;
		let mut to_server_mechanism=None;
		let mut from_server_mechanism=None;
		let mut neglect_busy_output = false;
		match_object_panic!(cv,"Basic",value,
			"virtual_channels" => match value
			{
				&ConfigurationValue::Number(f) => virtual_channels=Some(f as usize),
				_ => panic!("bad value for virtual_channels"),
			},
			//"routing" => routing=Some(new_routing(value)),
			//"virtual_channel_policy" => virtual_channel_policy=Some(new_virtual_channel_policy(value)),
			"virtual_channel_policies" => match value
			{
				//&ConfigurationValue::Array(ref a) => virtual_channel_policies=Some(a.iter().map(|cv|new_virtual_channel_policy(cv,plugs)).collect()),
				&ConfigurationValue::Array(ref a) => virtual_channel_policies=Some(a.iter().map(
					|cv|new_virtual_channel_policy(VCPolicyBuilderArgument{
					cv,
					plugs
				})).collect()),
				_ => panic!("bad value for permute"),
			}
			"delay" => (),//FIXME: yet undecided if/how to implement this.
			"buffer_size" => match value
			{
				&ConfigurationValue::Number(f) => buffer_size=Some(f as usize),
				_ => panic!("bad value for buffer_size"),
			},
			"output_buffer_size" => match value
			{
				&ConfigurationValue::Number(f) => output_buffer_size=Some(f as usize),
				_ => panic!("bad value for buffer_size"),
			},
			"bubble" => match value
			{
				&ConfigurationValue::True => bubble=Some(true),
				&ConfigurationValue::False => bubble=Some(false),
				_ => panic!("bad value for bubble"),
			},
			"flit_size" => match value
			{
				&ConfigurationValue::Number(f) => flit_size=Some(f as usize),
				_ => panic!("bad value for flit_size"),
			},
			"intransit_priority" => match value
			{
				&ConfigurationValue::True => intransit_priority=Some(true),
				&ConfigurationValue::False => intransit_priority=Some(false),
				_ => panic!("bad value for intransit_priority"),
			},
			"allow_request_busy_port" => match value
			{
				&ConfigurationValue::True => allow_request_busy_port=Some(true),
				&ConfigurationValue::False => allow_request_busy_port=Some(false),
				_ => panic!("bad value for allow_request_busy_port"),
			},
			"output_priorize_lowest_label" =>
			{
				eprintln!("WARNING: the name output_priorize_lowest_label is deprecated. Now the correct name is output_prioritize_lowest_label");
				match value
				{
					&ConfigurationValue::True => output_prioritize_lowest_label=Some(true),
					&ConfigurationValue::False => output_prioritize_lowest_label=Some(false),
					_ => panic!("bad value for output_prioritize_lowest_label"),
				}
			},
			"output_prioritize_lowest_label" => match value
			{
				&ConfigurationValue::True => output_prioritize_lowest_label=Some(true),
				&ConfigurationValue::False => output_prioritize_lowest_label=Some(false),
				_ => panic!("bad value for output_prioritize_lowest_label"),
			},
			"neglect_busy_output" => neglect_busy_output = value.as_bool().expect("bad value for neglect_busy_output"),
			"transmission_mechanism" => match value
			{
				&ConfigurationValue::Literal(ref s) => transmission_mechanism = Some(s.to_string()),
				_ => panic!("bad value for transmission_mechanism"),
			},
			"to_server_mechanism" => match value
			{
				&ConfigurationValue::Literal(ref s) => to_server_mechanism = Some(s.to_string()),
				_ => panic!("bad value for to_server_mechanism"),
			},
			"from_server_mechanism" => match value
			{
				&ConfigurationValue::Literal(ref s) => from_server_mechanism = Some(s.to_string()),
				_ => panic!("bad value for from_server_mechanism"),
			},
		);
		//let sides=sides.expect("There were no sides");
		let virtual_channels=virtual_channels.expect("There were no virtual_channels");
		let virtual_channel_policies=virtual_channel_policies.expect("There were no virtual_channel_policies");
		//let routing=routing.expect("There were no routing");
		let buffer_size=buffer_size.expect("There were no buffer_size");
		let output_buffer_size=output_buffer_size.expect("There were no output_buffer_size");
		let bubble=bubble.expect("There were no bubble");
		let flit_size=flit_size.expect("There were no flit_size");
		let intransit_priority=intransit_priority.expect("There were no intransit_priority");
		let allow_request_busy_port=allow_request_busy_port.expect("There were no allow_request_busy_port");
		let output_prioritize_lowest_label=output_prioritize_lowest_label.expect("There were no output_prioritize_lowest_label");
		let input_ports=topology.ports(router_index);
		let selected_input=(0..input_ports).map(|_|
			(0..virtual_channels).map(|_|None).collect()
		).collect();
		let selected_output=(0..input_ports).map(|_|
			(0..virtual_channels).map(|_|None).collect()
		).collect();
		let time_at_input_head=(0..input_ports).map(|_|
			(0..virtual_channels).map(|_|0).collect()
		).collect();
		let transmission_mechanism = transmission_mechanism.unwrap_or_else(||"SimpleVirtualChannels".to_string());
		//let from_server_mechanism = from_server_mechanism.unwrap_or_else(||"TransmissionFromServer".to_string());
		let from_server_mechanism = from_server_mechanism.unwrap_or_else(||"SimpleVirtualChannels".to_string());
		let to_server_mechanism = to_server_mechanism.unwrap_or_else(||"TransmissionToServer".to_string());
		//let transmission_mechanism = super::SimpleVirtualChannels::new(virtual_channels,buffer_size,flit_size);
		let transmission_builder_argument = TransmissionMechanismBuilderArgument{name:"",virtual_channels,buffer_size,size_to_send:flit_size};
		let transmission_mechanism = new_transmission_mechanism(TransmissionMechanismBuilderArgument{name:&transmission_mechanism,..transmission_builder_argument});
		let to_server_mechanism = new_transmission_mechanism(TransmissionMechanismBuilderArgument{name:&to_server_mechanism,..transmission_builder_argument});
		//let from_server_mechanism = TransmissionFromServer::new(virtual_channels,buffer_size,flit_size);
		let from_server_mechanism = new_transmission_mechanism(TransmissionMechanismBuilderArgument{name:&from_server_mechanism,..transmission_builder_argument});
		let transmission_port_status:Vec<Box<dyn StatusAtEmissor>> = (0..input_ports).map(|p|
			if let (Location::ServerPort(_server),_link_class)=topology.neighbour(router_index,p)
			{
				//let b:Box<dyn StatusAtEmissor> = Box::new(to_server_mechanism.new_status_at_emissor());
				//b
				to_server_mechanism.new_status_at_emissor()
			}
			else
			{
				//Box::new(transmission_mechanism.new_status_at_emissor())
				transmission_mechanism.new_status_at_emissor()
			}
		).collect();
		let reception_port_space = (0..input_ports).map(|p|
			if let (Location::ServerPort(_server),_link_class)=topology.neighbour(router_index,p)
			{
				//let b:Box<dyn SpaceAtReceptor> = Box::new(from_server_mechanism.new_space_at_receptor());
				//b
				from_server_mechanism.new_space_at_receptor()
			}
			else
			{
				//Box::new(transmission_mechanism.new_space_at_receptor())
				transmission_mechanism.new_space_at_receptor()
			}
		).collect();
		let output_buffers= if output_buffer_size==0 {vec![]} else{
			(0..input_ports).map(|_|
				(0..virtual_channels).map(|_|AugmentedBuffer::new()).collect()
			).collect()
		};
		let r=Rc::new(RefCell::new(Basic{
			self_rc: Weak::new(),
			next_events: vec![],
			last_process_at_cycle: None,
			router_index,
			//routing,
			virtual_channel_policies,
			bubble,
			flit_size,
			intransit_priority,
			allow_request_busy_port,
			output_prioritize_lowest_label,
			neglect_busy_output,
			buffer_size,
			transmission_port_status,
			reception_port_space,
			//transmission_mechanism,
			//to_server_mechanism,
			from_server_mechanism,
			output_buffer_size,
			output_buffers,
			selected_input,
			selected_output,
			time_at_input_head,
			output_arbiter: OutputArbiter::Token{port_token: vec![0;input_ports]},
			maximum_packet_size,
			//statistics_begin_cycle: 0,
			//statistics_output_buffer_occupation_per_vc: vec![0f64;virtual_channels],
			//statistics_reception_space_occupation_per_vc: vec![0f64;virtual_channels],
			statistics_temporal_step,
			principal_measurement: BasicRouterMeasurement::new(virtual_channels),
			temporal_statistics: vec![],
		}));
		//r.borrow_mut().self_rc=r.downgrade();
		r.borrow_mut().self_rc=Rc::<_>::downgrade(&r);
		r
	}
}

impl Basic
{
	///Whether a phit in an input buffer can advance.
	///bubble_in_use should be true only for leading phits that require the additional space.
	fn can_phit_advance(&self, phit:&Rc<Phit>, exit_port:usize, exit_vc:usize, bubble_in_use:bool)->bool
	{
		//if not internal output space
		if self.output_buffer_size==0
		{
			let status=&self.transmission_port_status[exit_port];
			if bubble_in_use
			{
				//status.can_transmit_whole_packet(&phit,exit_vc)
				if let Some(space)=status.known_available_space_for_virtual_channel(exit_vc)
				{
					status.can_transmit(phit,exit_vc) && space>= phit.packet.size + self.maximum_packet_size
				}
				else
				{
					panic!("Basic router requires knowledge of available space to apply bubble.");
				}
			}
			else
			{
				self.transmission_port_status[exit_port].can_transmit(phit,exit_vc)
			}
		}
		else
		{
			let available_internal_space = self.output_buffer_size-self.output_buffers[exit_port][exit_vc].len();
			let mut necessary_credits=1;
			if phit.is_begin()
			{
				//necessary_credits=self.counter.flit_size;
				//necessary_credits=match transmit_auxiliar_info
				necessary_credits=if bubble_in_use
				{
					phit.packet.size + self.maximum_packet_size
				}
				else
				{
					self.flit_size
				}
			}
			available_internal_space >= necessary_credits
		}
	}
	fn get_current_temporal_measurement(&mut self, cycle:Time) -> Option<usize>
	{
		if self.statistics_temporal_step>0
		{
			let index : usize = (cycle / self.statistics_temporal_step).try_into().unwrap();
			if self.temporal_statistics.len() <= index
			{
				let vcs=self.num_virtual_channels();
				self.temporal_statistics.resize_with(index+1,||BasicRouterMeasurement::new(vcs));
				self.temporal_statistics[index].begin_cycle = index as Time*self.statistics_temporal_step;
			}
			Some(index)
		} else { None }
	}
	fn gather_cycle_statistics(&mut self, cycle:Time, cycles_span:Time)
	{
		let amount_virtual_channels=self.num_virtual_channels();
		let current_temporal_index = self.get_current_temporal_measurement(cycle);
		for port_space in self.reception_port_space.iter()
		{
			for vc in 0..amount_virtual_channels
			{
				//self.principal_measurement.reception_space_occupation_per_vc[vc]+=(port_space.occupied_dedicated_space(vc).unwrap_or(0)*cycles_span) as f64 / self.reception_port_space.len() as f64;
				let increment = (port_space.occupied_dedicated_space(vc).unwrap_or(0)*cycles_span as usize) as f64 / self.reception_port_space.len() as f64;
				self.principal_measurement.reception_space_occupation_per_vc[vc]+= increment;
				if let Some(mindex)=current_temporal_index
				{
					//FIXME: what if cycles_span > 1
					self.temporal_statistics[mindex].reception_space_occupation_per_vc[vc]+= increment;
				}
			}
		}
		for output_port in self.output_buffers.iter()
		{
			for (vc,buffer) in output_port.iter().enumerate()
			{
				//self.principal_measurement.output_buffer_occupation_per_vc[vc]+=(buffer.len()*cycles_span) as f64 / self.output_buffers.len() as f64;
				let increment = (buffer.len()*cycles_span as usize) as f64 / self.output_buffers.len() as f64;
				self.principal_measurement.output_buffer_occupation_per_vc[vc]+= increment;
				if let Some(mindex)=current_temporal_index
				{
					//FIXME: what if cycles_span > 1
					self.temporal_statistics[mindex].output_buffer_occupation_per_vc[vc]+= increment;
				}
			}
		}
	}
}


///A phit in the virtual channel `virtual_channel` of the port `entry_port` is requesting to go to the virtual channel `requested_vc` of the port `requested_port`.
///The label is the one returned by the routing algorithm or 0 if it comes from a selection in a previous cycle.
#[derive(Clone)]
struct PortRequest
{
	packet: PacketRef,
	entry_port: usize,
	entry_vc: usize,
	requested_port: usize,
	requested_vc: usize,
	label: i32,
}

impl Eventful for Basic
{
	///main routine of the router. Do all things that must be done in a cycle, if any.
	fn process(&mut self, simulation:&SimulationShared, mutable:&mut SimulationMut) -> Vec<EventGeneration>
	{
		let mut cycles_span = 1;//cycles since last checked
		if let Some(ref last)=self.last_process_at_cycle
		{
			cycles_span = simulation.cycle - *last;
			if *last >= simulation.cycle
			{
				panic!("Trying to process at cycle {} a router::Basic already processed at {}",simulation.cycle,last);
			}
			//if *last +1 < simulation.cycle
			//{
			//	println!("INFO: {} cycles since last processing router {}, cycle={}",simulation.cycle-*last,self.router_index,simulation.cycle);
			//}
		}
		self.last_process_at_cycle = Some(simulation.cycle);
		let mut request:Vec<PortRequest>=vec![];
		let topology = simulation.network.topology.as_ref();
		
		let amount_virtual_channels=self.num_virtual_channels();
		//-- gather cycle statistics
		self.gather_cycle_statistics(simulation.cycle,cycles_span);

		//-- Precompute whatever polcies ask for.
		let server_ports : Option<Vec<usize>> = if self.virtual_channel_policies.iter().any(|policy|policy.need_server_ports())
		{
			Some((0..topology.ports(self.router_index)).filter(|&p|
				matches!(topology.neighbour(self.router_index,p), (Location::ServerPort(_server),_link_class))
			).collect())
		}
		else
		{
			None
		};
		// `busy_ports[port_index]` is true for output ports for which there is some assigned input buffer which can advance.
		let busy_ports:Vec<bool> = self.transmission_port_status.iter().enumerate().map(|(port, _status)|{
			let mut is_busy = false;
			for vc in 0..amount_virtual_channels
			{
				if let Some((ref _packet,selected_port,selected_virtual_channel))=self.selected_input[port][vc]
				{
					if let Some(phit)=self.reception_port_space[selected_port].front_virtual_channel(selected_virtual_channel)
					{
						//if status.can_transmit(&phit,vc,None)
						if self.can_phit_advance(&phit,port,vc,false)
						{
							is_busy=true;
							break;
						}
					}
				}
			}
			is_busy
		}).collect();
		let port_last_transmission:Option<Vec<Time>> = if self.virtual_channel_policies.iter().any(|policy|policy.need_port_last_transmission())
		{
			Some(self.transmission_port_status.iter().map(|p|
				//p.iter().map(|ref vp|vp.last_transmission).max().unwrap()
				p.get_last_transmission()
			).collect())
		}
		else
		{
			None
		};
		let port_average_neighbour_queue_length:Option<Vec<f32>> = if self.virtual_channel_policies.iter().any(|policy|policy.need_port_average_queue_length())
		{
			Some(self.transmission_port_status.iter().map(|p|{
				//let total=p.iter().map(|ref vp|self.buffer_size - vp.neighbour_credits).sum::<usize>();
				//(total as f32) / (p.len() as f32)
				let total=(0..amount_virtual_channels).map(|vc|{
					//self.buffer_size-p.known_available_space_for_virtual_channel(vc).expect("needs to know available space")
					let available = p.known_available_space_for_virtual_channel(vc).expect("needs to know available space");
					if available>self.buffer_size
					{
						//panic!("We should never have more available space than the buffer size.");
						//Actually when the neighbour is a server it may have longer queue.
						0
					}
					else
					{
						self.buffer_size - available
					}
				}).sum::<usize>();
				(total as f32) / (amount_virtual_channels as f32)
			}).collect())
		}
		else
		{
			None
		};
		//let average_neighbour_queue_length:Option<f32> = if let Some(ref v)=port_average_neighbour_queue_length
		//{
		//	Some(v.iter().sum::<f32>() / (v.len() as f32))
		//}
		//else
		//{
		//	None
		//};
		let port_occupied_output_space:Option<Vec<usize>> = if self.output_buffer_size==0
		{
			None
		}
		else
		{
			Some(self.output_buffers.iter().map(|p|
				p.iter().map(|b|b.len()).sum()
			).collect())
		};
		let port_available_output_space:Option<Vec<usize>> = if self.output_buffer_size==0
		{
			None
		}
		else
		{
			Some(self.output_buffers.iter().map(|p|
				p.iter().map(|b|self.output_buffer_size - b.len()).sum()
			).collect())
		};
		let virtual_channel_occupied_output_space:Option<Vec<Vec<usize>>> = if self.output_buffer_size==0
		{
			None
		}
		else
		{
			Some(self.output_buffers.iter().map(|p|
				p.iter().map(|b|b.len()).collect()
			).collect())
		};
		let virtual_channel_available_output_space:Option<Vec<Vec<usize>>> = if self.output_buffer_size==0
		{
			None
		}
		else
		{
			Some(self.output_buffers.iter().map(|p|
				p.iter().map(|b|self.output_buffer_size-b.len()).collect()
			).collect())
		};

		//-- Routing and requests.
		let mut undecided_channels=0;//just as indicator if the router has pending work.
		let mut moved_phits=0;//another indicator of pending work.
		//Iterate over the reception space to find phits that request to advance.
		for entry_port in 0..self.reception_port_space.len()
		{
			for phit in self.reception_port_space[entry_port].front_iter()
			{
				let entry_vc={
					phit.virtual_channel.borrow().expect("it should have an associated virtual channel")
				};
				self.time_at_input_head[entry_port][entry_vc]+=1;
				//let (requested_port,requested_vc,label)=
				match self.selected_output[entry_port][entry_vc]
				{
					None =>
					{
						undecided_channels+=1;
						let target_server=phit.packet.message.destination;
						let (target_location,_link_class)=topology.server_neighbour(target_server);
						let target_router=match target_location
						{
							Location::RouterPort{router_index,router_port:_} =>router_index,
							_ => panic!("The server is not attached to a router"),
						};
						let routing_candidates=simulation.routing.next(phit.packet.routing_info.borrow().deref(),simulation.network.topology.as_ref(),self.router_index,target_router,Some(target_server),amount_virtual_channels,&mut mutable.rng).unwrap_or_else(|e|panic!("Error {} while routing.",e));
						let routing_idempotent = routing_candidates.idempotent;
						if routing_candidates.len()==0
						{
							if routing_idempotent
							{
								panic!("There are no choices for packet {:?} entry_port={} entry_vc={} in router {} towards server {}",phit.packet,entry_port,entry_vc,self.router_index,target_server);
							}
							//There are currently no good port choices, but there may be in the future.
							continue;
						}
						let mut good_ports=routing_candidates.into_iter().filter_map(|candidate|{
							let CandidateEgress{port:f_port,virtual_channel:f_virtual_channel,..} = candidate;
							//We analyze each candidate output port, considering whether they are in use (port or virtual channel).
							match self.selected_input[f_port][f_virtual_channel]
							{
								//Some((s_port,s_virtual_channel))=> s_port==entry_port && s_virtual_channel==entry_vc,
								//Some(_) => None,
								// Keep these candidates until EnforceFlowControl, so policies have all information.
								Some(_) => if self.neglect_busy_output {None} else {Some(CandidateEgress{router_allows:Some(false), ..candidate})},
								None =>
								{
									let bubble_in_use= self.bubble && phit.is_begin() && simulation.network.topology.is_direction_change(self.router_index,entry_port,f_port);
									//if self.transmission_port_status[f_port].can_transmit(&phit,f_virtual_channel,transmit_auxiliar_info)
									let allowed = if self.can_phit_advance(&phit,f_port,f_virtual_channel,bubble_in_use)
									{
										if self.allow_request_busy_port
										{
											true
										}
										else
										{
											!busy_ports[f_port]
										}
									}
									else
									{
										false
									};
									Some(CandidateEgress{router_allows:Some(allowed), ..candidate})
								}
							}
						}).collect::<Vec<_>>();
						let performed_hops=phit.packet.routing_info.borrow().hops;
						//Apply all the declared virtual channel policies in order.
						let request_info=RequestInfo{
							target_router_index: target_router,
							entry_port,
							entry_virtual_channel: entry_vc,
							performed_hops,
							server_ports: server_ports.as_ref(),
							port_average_neighbour_queue_length: port_average_neighbour_queue_length.as_ref(),
							port_last_transmission: port_last_transmission.as_ref(),
							port_occupied_output_space: port_occupied_output_space.as_ref(),
							port_available_output_space: port_available_output_space.as_ref(),
							virtual_channel_occupied_output_space: virtual_channel_occupied_output_space.as_ref(),
							virtual_channel_available_output_space: virtual_channel_available_output_space.as_ref(),
							time_at_front: Some(self.time_at_input_head[entry_port][entry_vc]),
							current_cycle: simulation.cycle,
							phit: phit.clone(),
						};
						for vcp in self.virtual_channel_policies.iter()
						{
							//good_ports=vcp.filter(good_ports,self,target_router,entry_port,entry_vc,performed_hops,&server_ports,&port_average_neighbour_queue_length,&port_last_transmission,&port_occupied_output_space,&port_available_output_space,simulation.cycle,topology,&mutable.rng);
							good_ports=vcp.filter(good_ports,self,&request_info,topology,&mut mutable.rng);
							if good_ports.is_empty()
							{
								break;//No need to check other policies.
							}
						}
						if good_ports.is_empty()
						{
							continue;//There is no available port satisfying the policies. Hopefully there will in the future.
						}
						//else if good_ports.len()>=2
						//{
						//	//TODO: this will not be true when having true allocators.
						//	panic!("You need a VirtualChannelPolicy able to select a single (port,vc).");
						//}
						//simulation.routing.performed_request(&good_ports[0],&phit.packet.routing_info,simulation.network.topology.as_ref(),self.router_index,target_server,amount_virtual_channels,&mutable.rng);
						//match good_ports[0]
						//{
						//	CandidateEgress{port,virtual_channel,label,estimated_remaining_hops:_,..}=>(port,virtual_channel,label),
						//}
						for candidate in good_ports.into_iter()
						{
							simulation.routing.performed_request(&candidate,&phit.packet.routing_info,simulation.network.topology.as_ref(),self.router_index,target_router,Some(target_server),amount_virtual_channels,&mut mutable.rng);
							let CandidateEgress{port:requested_port,virtual_channel:requested_vc,label,..} = candidate;
							if self.selected_input[requested_port][requested_vc].is_none()
							{
								request.push(PortRequest{packet:phit.packet.clone(),entry_port,entry_vc,requested_port,requested_vc,label});
							}
						}
					},
					Some((ref _packet,_port,_vc)) => (),
						//(port,vc,0),//FIXME: perhaps 0 changes into None?
				};
				// //FIXME: this should not call known_available_space_for_virtual_channel
				// //In wormhole we may have a selected output but be unable to advance, but it is not clear whether makes any difference.
				// let credits=self.transmission_port_status[requested_port].known_available_space_for_virtual_channel(requested_vc).expect("no available space known");
				// //println!("entry_port={} virtual_channel={} credits={}",entry_port,entry_vc,credits);
				// if credits>0
				// {
				// 	match self.selected_input[requested_port][requested_vc]
				// 	{
				// 		Some(_) => (),
				// 		None => request.push( PortRequest{packet:phit.packet.clone(),entry_port,entry_vc,requested_port,requested_vc,label} ),
				// 	};
				// }
			}
		}

		//-- Arbitrate the requests.
		let request_len = request.len();
		//FIXME: allocator policies
		let min_label= request.iter().map(|r|r.label).min().unwrap_or(0);
		let max_label=request.iter().map(|r|r.label).max().unwrap_or(0);
		//Split que sequence in subsequences, where any items in a subsequence has more priority than any element in a later subsequence.
		let request_sequence:Vec<Vec<PortRequest>>=if self.output_prioritize_lowest_label
		{
			//(min_label..max_label+1).map(|label|request.iter().filter(|r|r.label==label).map(|&t|t).collect()).collect()
			//(min_label..max_label+1).map(move |label|request.into_iter().filter(|r|r.label==label).collect()).collect()
			let mut sequence : Vec<Vec<PortRequest>> = vec![ Vec::with_capacity(request.len()) ; (max_label+1-min_label) as usize];
			for req in request.into_iter()
			{
				let index :usize = (req.label - min_label) as usize;
				sequence[index].push(req);
			}
			sequence
		}
		else
		{
			vec![request]
		};
		//Shuffle the subsequences. XXX Perhaps the separation transit/injection should be done in a similar as to the separation by labels.
		//for ref mut rx in request_sequence.iter_mut()
		let captured_intransit_priority=self.intransit_priority;//to move into closure
		let captured_router_index=self.router_index;//to move into closure
		let request_it = request_sequence.into_iter().flat_map(|mut rx|{
			if captured_intransit_priority
			{
				//let (mut request_transit, mut request_injection) : (Vec<PortRequest>,Vec<PortRequest>) = rx.into_iter().map(|&mut t|t).partition(|&req|{
				//	match simulation.network.topology.neighbour(self.router_index,req.entry_port)
				//	{
				//		( Location::RouterPort{..} ,_) => true,
				//		_ => false,
				//	}
				//});
				let (mut request_transit, mut request_injection) : (Vec<PortRequest>,Vec<PortRequest>) = rx.into_iter().partition(|req|{
					matches!(simulation.network.topology.neighbour(captured_router_index,req.entry_port), ( Location::RouterPort{..} ,_))
				});
				//shuffle has changed notably from rand-0.4 to rand-0.8
				//mutable.rng.borrow_mut().shuffle(&mut request_transit);
				//mutable.rng.borrow_mut().shuffle(&mut request_injection);
				let rng=&mut mutable.rng;
				request_transit.shuffle(rng);
				request_injection.shuffle(rng);
				//**rx=request_transit;
				rx=request_transit;
				rx.append(&mut request_injection);
			}
			else
			{
				//shuffle has changed notably from rand-0.4 to rand-0.8
				//mutable.rng.borrow_mut().shuffle(&mut rx);
				rx.shuffle(&mut mutable.rng);
			}
			rx
		});
		//Complete the arbitration of the requests by writing the selected_input of the output virtual ports.
		//let request=request_sequence.concat();
		for PortRequest{packet,entry_port,entry_vc,requested_port,requested_vc,..} in request_it
		{
			//println!("processing request {},{},{},{}",entry_port,entry_vc,requested_port,requested_vc);
			if self.selected_input[requested_port][requested_vc].is_none()
			{
				if self.selected_output[entry_port][entry_vc].is_none()
				{
					self.selected_input[requested_port][requested_vc]=Some((packet.clone(),entry_port,entry_vc));
					self.selected_output[entry_port][entry_vc] = Some((packet,requested_port,requested_vc));
				}
			};
		}

		//-- For each output port decide which input actually uses it this cycle.
		let mut events=vec![];
		for exit_port in 0..self.transmission_port_status.len()
		{
			let nvc=amount_virtual_channels;
			//Gather the list of all vc that can advance
			let mut cand=Vec::with_capacity(nvc);
			let mut cand_in_transit=false;
			let mut undo_selected_input=Vec::with_capacity(nvc);
			for exit_vc in 0..nvc
			{
				if let Some((ref entry_packet,entry_port,entry_vc))=self.selected_input[exit_port][exit_vc]
				{
					if self.output_buffer_size>0
					{
						//-- Move phits into the internal output space
						//Note that it is possible when flit_size<packet_size for the packet to not be in that buffer. The output arbiter can decide to advance other virtual channel.
						if let Ok((phit,ack_message)) = self.reception_port_space[entry_port].extract(entry_vc)
						{
							if self.output_buffers[exit_port][exit_vc].len()>=self.output_buffer_size
							{
								panic!("Trying to move into a full output buffer.");
							}
							moved_phits+=1;
							self.time_at_input_head[entry_port][entry_vc]=0;
							*phit.virtual_channel.borrow_mut()=Some(exit_vc);
							if let Some(message)=ack_message
							{
								let (previous_location,previous_link_class)=simulation.network.topology.neighbour(self.router_index,entry_port);
								events.push(EventGeneration{
									delay: simulation.link_classes[previous_link_class].delay,
									position:CyclePosition::Begin,
									//event:Event::Acknowledge{location:previous_location,message:AcknowledgeMessage::ack_phit_clear_from_virtual_channel(entry_vc)},
									event:Event::Acknowledge{location:previous_location,message},
								});
							}
							if let Some((ref s_exit_packet,s_exit_port,s_exit_vc))=self.selected_output[entry_port][entry_vc]
							{
								let entry_packet_ptr = entry_packet.as_ref() as *const Packet;
								let s_exit_packet_ptr = s_exit_packet.as_ref() as *const Packet;
								if s_exit_packet_ptr!=entry_packet_ptr || s_exit_port!=exit_port || s_exit_vc!=exit_vc
								{
									panic!("Mismatch between selected input and selected output: selected_input[{}][{}]=({:?},{},{}) selected_output[{}][{}]=({:?},{},{}).",exit_port,exit_vc,entry_packet_ptr,entry_port,entry_vc,  entry_port,entry_vc,s_exit_packet_ptr,s_exit_port,s_exit_vc);
								}
							}
							if phit.is_end()
							{
								self.selected_input[exit_port][exit_vc]=None;
								self.selected_output[entry_port][entry_vc]=None;
							}
							else
							{
								self.selected_output[entry_port][entry_vc]=Some((entry_packet.clone(),exit_port,exit_vc));
							}
							self.output_buffers[exit_port][exit_vc].push(phit,(entry_port,entry_vc));
						}
						else
						{
							if self.flit_size>1
							{
								//We would like to panic if phit.packet.size<=flit_size, but we do not have the phit accesible.
								println!("WARNING: There were no phit at the selected_input[{}][{}]=({},{}) of the router {}.",exit_port,exit_vc,entry_port,entry_vc,self.router_index);
							}
						}
					}
					else if let Some(phit)=self.reception_port_space[entry_port].front_virtual_channel(entry_vc)
					{
						if phit.is_begin()
						{
							undo_selected_input.push(exit_vc);
						}
						let bubble_in_use= self.bubble && phit.is_begin() && simulation.network.topology.is_direction_change(self.router_index,entry_port,exit_port);
						//if self.transmission_port_status[exit_port].can_transmit(&phit,exit_vc,transmit_auxiliar_info)
						if self.can_phit_advance(&phit,exit_port,exit_vc,bubble_in_use)
						{
							//cand.push(exit_vc);
							if cand_in_transit
							{
								if !phit.is_begin()
								{
									cand.push(exit_vc);
								}
							}
							else
							{
								if phit.is_begin()
								{
									cand.push(exit_vc);
								}
								else
								{
									cand=vec![exit_vc];
									cand_in_transit=true;
								}
							}
						}
					}
				}
				if self.output_buffer_size>0
				{
					//Candidates when using output ports.
					if let Some( (phit,(entry_port,_entry_vc))) = self.output_buffers[exit_port][exit_vc].front()
					{
						let bubble_in_use= self.bubble && phit.is_begin() && simulation.network.topology.is_direction_change(self.router_index,entry_port,exit_port);
						let status=&self.transmission_port_status[exit_port];
						let can_transmit = if bubble_in_use
						{
							//self.transmission_port_status[exit_port].can_transmit_whole_packet(&phit,exit_vc)
							if let Some(space)=status.known_available_space_for_virtual_channel(exit_vc)
							{
								status.can_transmit(&phit,exit_vc) && space>= phit.packet.size + self.maximum_packet_size
							}
							else
							{
								panic!("Basic router requires knowledge of available space to apply bubble.");
							}
						}
						else
						{
							status.can_transmit(&phit,exit_vc)
						};
						if can_transmit
						{
							if cand_in_transit
							{
								if !phit.is_begin()
								{
									cand.push(exit_vc);
								}
							}
							else
							{
								if phit.is_begin()
								{
									cand.push(exit_vc);
								}
								else
								{
									cand=vec![exit_vc];
									cand_in_transit=true;
								}
							}
						}
						else
						{
							if 0<phit.index && phit.index<self.flit_size
							{
								panic!("cannot transmit phit (index={}) but it should (flit_size={})",phit.index,self.flit_size);
							}
						}
					}
				}
			}
			//for selected_virtual_channel in 0..nvc
			let selected_virtual_channel = if !cand.is_empty()
			{
				//Then select one of the vc candidates (either in input or output buffer) to actually use the physical port.
				let selected_virtual_channel = match self.output_arbiter
				{
					//OutputArbiter::Random=> cand[mutable.rng.borrow_mut().gen_range(0,cand.len())],//rand-0.4
					OutputArbiter::Random=> cand[mutable.rng.gen_range(0..cand.len())],//rand-0.8
					OutputArbiter::Token{ref mut port_token}=>
					{
						//Or by tokens as in fsin
						//let nvc=self.virtual_ports[exit_port].len() as i64;
						let nvc= amount_virtual_channels as i64;
						let token= port_token[exit_port] as i64;
						let mut best=0;
						let mut bestd=nvc;
						for vc in cand
						{
							let mut d:i64 = vc as i64 - token;
							if d<0
							{
								d+=nvc;
							}
							if d<bestd
							{
								best=vc;
								bestd=d;
							}
						}
						port_token[exit_port]=best;
						best
					},
				};
				//move phits around.
				let (phit,original_port) = if self.output_buffer_size>0
				{
					//If we get the phit from an output buffer there is little to do.
					let (phit,(entry_port,_entry_vc))=self.output_buffers[exit_port][selected_virtual_channel].pop().expect("incorrect selected_input");
					(phit,entry_port)
				}
				else
				{
					//If we get the phit from an input buffer we have to send acks to the previous router and take care of sending the packet in one piece.
					if let Some((ref packet,iport,entry_vc))=self.selected_input[exit_port][selected_virtual_channel]
					{
						if let Ok((phit,ack_message)) = self.reception_port_space[iport].extract(entry_vc)
						{
							moved_phits+=1;
							self.time_at_input_head[iport][entry_vc]=0;
							//phit.virtual_channel.replace(Some(selected_virtual_channel));
							*phit.virtual_channel.borrow_mut()=Some(selected_virtual_channel);
							if let Some(message)=ack_message
							{
								let (previous_location,previous_link_class)=simulation.network.topology.neighbour(self.router_index,iport);
								events.push(EventGeneration{
									delay: simulation.link_classes[previous_link_class].delay,
									position:CyclePosition::Begin,
									//event:Event::PhitClearAcknowledge{location:previous_location,virtual_channel:entry_vc},
									event:Event::Acknowledge{location:previous_location,message},
								});
							}
							if phit.is_end()
							{
								self.selected_input[exit_port][selected_virtual_channel]=None;
								self.selected_output[iport][entry_vc]=None;
							}
							else
							{
								self.selected_output[iport][entry_vc]=Some((packet.clone(),exit_port,selected_virtual_channel));
							}
							(phit,iport)
						}
						else
						{
							panic!("There were no phit at the selected_input[{}][{}]=({},{}), and somehow it is selected",exit_port,selected_virtual_channel,iport,entry_vc);
						}
					}
					else
					{
						panic!("incorrect selected_input")
					}
				};
				let (new_location,link_class)=simulation.network.topology.neighbour(self.router_index,exit_port);
				//Send the phit to the other link endpoint.
				events.push(EventGeneration{
					delay: simulation.link_classes[link_class].delay,
					position:CyclePosition::Begin,
					event:Event::PhitToLocation{
						phit: phit.clone(),
						previous: Location::RouterPort{
							router_index: self.router_index,
							router_port: original_port,
						},
						new: new_location,
					},
				});
				self.transmission_port_status[exit_port].notify_outcoming_phit(selected_virtual_channel,simulation.cycle);
				if phit.is_end()
				{
					if let OutputArbiter::Token{ref mut port_token}=self.output_arbiter
					{
						port_token[exit_port]=(port_token[exit_port]+1)%amount_virtual_channels;
					}
				}
				Some(selected_virtual_channel)
			} else {None};
			for other_virtual_channel in undo_selected_input
			{
				if Some(other_virtual_channel) != selected_virtual_channel
				{
					//Packets that have not started to move can change their decision at the next cycle
					self.selected_input[exit_port][other_virtual_channel]=None;
				}
			}
		}
		self.next_events.pop();//remove the event that was served.
		//TODO: what to do with probabilistic requests???
		if undecided_channels>0 || moved_phits>0 || !events.is_empty() || request_len>0
		//if undecided_channels>0 || moved_phits>0 || events.len()>0
		//if true
		{
			//Repeat at next cycle
			if let Some(event) = self.schedule(simulation.cycle,1)
			{
				events.push(event);
			}
		}
		events
	}
	//fn pending_events(&self)->usize
	//{
	//	if self.event_pending { 1 } else { 0 }
	//}
	//fn add_pending_event(&mut self)
	//{
	//	self.event_pending=true;
	//}
	//fn clear_pending_events(&mut self)
	//{
	//	self.event_pending=false;
	//}
	fn as_eventful(&self)->Weak<RefCell<dyn Eventful>>
	{
		self.self_rc.clone()
	}
	fn schedule(&mut self, current_cycle:Time, delay:Time) -> Option<EventGeneration>
	{
		let target = current_cycle+delay;
		if self.next_events.is_empty() || target<*self.next_events.last().unwrap() {
			self.next_events.push(target);
			let event = Event::Generic(self.as_eventful().upgrade().expect("missing component"));
			Some(EventGeneration{
				delay: target-current_cycle,
				position: CyclePosition::End,
				event,
			})
		} else {
			None
		}
	}
}

impl Quantifiable for Basic
{
	fn total_memory(&self) -> usize
	{
		//FIXME: redo
		//return size_of::<Basic<TM>>() + self.virtual_ports.total_memory() + self.port_token.total_memory();
		return size_of::<Basic>();
	}
	fn print_memory_breakdown(&self)
	{
		unimplemented!();
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}

