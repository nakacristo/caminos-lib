use std::cell::RefCell;
use std::rc::{Rc,Weak};
use std::ops::Deref;
use std::mem::size_of;
use ::rand::{Rng,rngs::StdRng};
use super::{Router,AbstractTransmissionMechanism,TransmissionMechanismBuilderArgument,new_transmission_mechanism,StatusAtEmissor,SpaceAtReceptor,AugmentedBuffer,AcknowledgeMessage};
use crate::allocator::{Allocator,VCARequest,AllocatorBuilderArgument, new_allocator};
use crate::config_parser::ConfigurationValue;
use crate::router::RouterBuilderArgument;
use crate::topology::{Location,Topology};
use crate::routing::CandidateEgress;
use crate::policies::{RequestInfo,VirtualChannelPolicy,new_virtual_channel_policy,VCPolicyBuilderArgument};
use crate::event::{self,Event,Eventful,EventGeneration,CyclePosition,Time};
use crate::{Phit,SimulationShared,SimulationMut};
use crate::quantify::Quantifiable;
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
pub struct InputOutput
{
	///Weak pointer to itself, see <https://users.rust-lang.org/t/making-a-rc-refcell-trait2-from-rc-refcell-trait1/16086/3>
	self_rc: Weak<RefCell<InputOutput>>,
	///When is the next scheduled event. Stack with the soonner event the last.
	next_events: Vec<Time>,
	///The cycle number of the last time InputOutput::process was called. Only for debugging/assertion purposes.
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
	///Delay in cycles to traverse the crossbar. In pipeline.
	crossbar_delay: Time,
	///Give priority to in-transit packets over packets in injection queues.
	intransit_priority: bool,
	///To allow to request a port even if some other packet is being transmitted throught it to a different virtual channel (as FSIN does).
	///It may appear that should obviously be put to `true`, but in practice that just reduces performance.
	allow_request_busy_port: bool,
	///Whether to immediately discard candidate outputs when they are currently receiving from an input.
	///Otherwise, these candidates are marked as impossible, but they can be processed by the `virtual_channel_policies`.
	///In particular, [EnforceFlowControl] will filter them out.
	///Defaults to false.
	neglect_busy_output: bool,
	/// `transmission_port_status[port] = status`
	transmission_port_status: Vec<Box<dyn StatusAtEmissor>>,
	/// `reception_port_space[port] = space`
	reception_port_space: Vec<Box<dyn SpaceAtReceptor>>,
	/// The server to router mechanism employed.
	/// This will be used to build the status at the servers.
	from_server_mechanism: Box<dyn AbstractTransmissionMechanism>,
	///if greater than 0 then the size of each of them, else BAD!
	output_buffer_size: usize,
	///The outut buffers indexed as `[output_port][output_vc]`.
	///Phits are stored with their `(entry_port,entry_vc)`.
	output_buffers: Vec<Vec<AugmentedBuffer<(usize,usize)>>>,
	///Number of phits currently being traversing the crossbar towards the output buffer.
	///Specifically, `output_buffer_phits_traversing_crossbar[output_port][output_vc]` counts phits that are going to be inserted
	///into `output_buffers[output_port][output_vc]` at some point.
	output_buffer_phits_traversing_crossbar: Vec<Vec<usize>>,
	output_schedulers: Vec<Rc<RefCell<internal::TryLinkTraversal>>>,
	///If not None then the input port+virtual_channel which is either sending by this port+virtual_channel or writing to this output buffer.
	///We keep the packet for debugging/check considerations.
	selected_input: Vec<Vec<Option<(usize,usize)>>>,
	///If not None then all the phits should go through this port+virtual_channel or stored in this output buffer, since they are part of the same packet
	///We keep the packet for debugging/check considerations.
	selected_output: Vec<Vec<Option<(usize,usize)>>>,
	///Number of cycles that the current phit, if any, in the head of a given (port,virtual channel) input buffer the phit has been waiting.
	time_at_input_head: Vec<Vec<usize>>,
	///An arbiter of the physical output port.
	output_arbiter: OutputArbiter,
	///The maximum packet size that is allowed. Only for bubble consideration, that reserves space for a given packet plus maximum packet size.
	maximum_packet_size: usize,
	///Divisor of the cycles in which the crossbar operates.
	///Without other overrides, the quotient `general_frequency_divisor/crossbar_frequency_divisor` is the internal speedup.
	crossbar_frequency_divisor: Time,

	///Metrics
	buffer_speed_metric: Option<Vec<Vec<TimeSegmentMetric>>>,


	//allocator:
	///The allocator for the croosbar.
	crossbar_allocator: Box<dyn Allocator>,
	//Use the labels provided by the routing to sort the petitions in the output arbiter.
	//output_priorize_lowest_label: bool, // USE RandomPriorityAllocator instead of this parameter.

	//statistics:
	///The first cycle included in the statistics.
	statistics_begin_cycle: Time,
	///Accumulated over time, averaged per port.
	statistics_output_buffer_occupation_per_vc: Vec<f64>,
	///Accumulated over time, averaged per port.
	statistics_reception_space_occupation_per_vc: Vec<f64>,
}

impl Router for InputOutput
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
		if let Some(measure) = self.buffer_speed_metric.as_mut() //To save the speed at which acks arrive...
		{
			let credits = if let Some(credits) = ack_message.set_available_size.as_ref() { *credits } else { 1 };
			let vc = *ack_message.virtual_channel.as_ref().expect("ack_message should have a virtual_channel");
			measure[port][vc].add_measure(credits as f64, current_cycle);
		}

		self.transmission_port_status[port].acknowledge(ack_message);

		let mut events = vec![];
		if let Some(event) = self.schedule(current_cycle,0) {
			events.push(event);
		}
		if let Some(event) = self.output_schedulers[port].borrow_mut().schedule(current_cycle,0) {
			events.push(event);
		}
		events
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
		let cycle_span = cycle - self.statistics_begin_cycle;
		let mut reception_space_occupation_per_vc:Option<Vec<f64>> = Some(self.statistics_reception_space_occupation_per_vc.iter().map(|x|x/cycle_span as f64).collect());
		let mut output_buffer_occupation_per_vc:Option<Vec<f64>> = Some(self.statistics_output_buffer_occupation_per_vc.iter().map(|x|x/cycle_span as f64).collect());
		if let Some(previous)=statistics
		{
			if let ConfigurationValue::Object(cv_name,previous_pairs) = previous
			{
				if cv_name!="InputOutput"
				{
					panic!("incompatible statistics, should be `InputOutput` object not `{}`",cv_name);
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
									println!("Ignoring average_output_buffer_occupation_per_vc.");
								}
							}
							_ => panic!("bad value for average_output_buffer_occupation_per_vc"),
						},
						_ => panic!("Nothing to do with field {} in InputOutput statistics",name),
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
		Some(ConfigurationValue::Object(String::from("InputOutput"),result_content))
	}

	fn reset_statistics(&mut self, next_cycle:Time)
	{
		self.statistics_begin_cycle=next_cycle;
		for x in self.statistics_output_buffer_occupation_per_vc.iter_mut()
		{
			*x=0f64;
		}
		for x in self.statistics_reception_space_occupation_per_vc.iter_mut()
		{
			*x=0f64;
		}
	}
	fn build_emissor_status(&self, port:usize, topology:&dyn Topology) -> Box<dyn StatusAtEmissor+'static>
	{
		if let (Location::ServerPort(_server),_link_class)=topology.neighbour(self.router_index,port)
		{
			self.from_server_mechanism.new_status_at_emissor()
		}
		else
		{
			unimplemented!()
		}
	}
}


impl InputOutput
{
	pub fn new(arg:RouterBuilderArgument) -> Rc<RefCell<InputOutput>>
	{
		let RouterBuilderArgument{
			router_index,
			cv,
			plugs,
			topology,
			maximum_packet_size,
			general_frequency_divisor,
			..
		} = arg;
		//let mut servers=None;
		//let mut load=None;
		let mut virtual_channels=None;
		let mut injection_buffers=None;
		//let mut routing=None;
		let mut buffer_size=None;
		let mut virtual_channel_policies=None;
		let mut bubble=None;
		let mut flit_size=None;
		let mut intransit_priority=None;
		let mut allow_request_busy_port=None;
//		let mut output_priorize_lowest_label=None;
		let mut output_buffer_size=None;
		let mut allocator_value=None;
		let mut transmission_mechanism=None;
		let mut to_server_mechanism=None;
		let mut from_server_mechanism=None;
		let mut crossbar_delay: Time =0;
		let mut neglect_busy_output = false;
		let mut crossbar_frequency_divisor = general_frequency_divisor;
		let mut time_segment_metric_buffer_rate = None;

		match_object_panic!(cv,["InputOutput","InputOutputMonocycle"],value,
			"virtual_channels" => match value
			{
				&ConfigurationValue::Number(f) => virtual_channels=Some(f as usize),
				_ => panic!("bad value for virtual_channels"),
			},
			"injection_buffers" => match value
			{
				&ConfigurationValue::Number(f) => injection_buffers=Some(f as usize),
				_ => panic!("bad value for injection_buffers"),
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
			"crossbar_delay" | "delay" => crossbar_delay = value.as_time().expect("bad value for crossbar_delay"),
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
/*					"output_priorize_lowest_label" => match value
			{
				&ConfigurationValue::True => output_priorize_lowest_label=Some(true),
				&ConfigurationValue::False => output_priorize_lowest_label=Some(false),
				_ => panic!("bad value for output_priorize_lowest_label"),
			};
*/
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
			"time_segment_metric_buffer_rate" => time_segment_metric_buffer_rate = Some(value.as_usize().expect("bad value for time_segment_metric_buffer_rate")),
			"allocator" => allocator_value=Some(value.clone()),
			"crossbar_frequency_divisor" => crossbar_frequency_divisor = value.as_time().expect("bad value for crossbar_frequency_divisor"),
		);
		//let sides=sides.expect("There were no sides");
		let virtual_channels=virtual_channels.expect("There were no virtual_channels");
		let injection_buffers = if let Some(i)=injection_buffers
		{
			i
		}
		else
		{
			virtual_channels
		};

		let virtual_channel_policies=virtual_channel_policies.expect("There were no virtual_channel_policies");
		//let routing=routing.expect("There were no routing");
		let buffer_size=buffer_size.expect("There were no buffer_size");
		let output_buffer_size=output_buffer_size.expect("There were no output_buffer_size");
		let bubble=bubble.expect("There were no bubble");
		let flit_size=flit_size.expect("There were no flit_size");
		let intransit_priority=intransit_priority.expect("There were no intransit_priority");
		let allow_request_busy_port=allow_request_busy_port.expect("There were no allow_request_busy_port");
//		let output_priorize_lowest_label=output_priorize_lowest_label.expect("There were no output_priorize_lowest_label");
		let input_ports=topology.ports(router_index);
		let allocator = new_allocator(AllocatorBuilderArgument{
			cv:&allocator_value.expect("There were no allocator"),
			num_clients:input_ports * virtual_channels,
			num_resources:input_ports * virtual_channels,
			plugs,
			rng:arg.rng,
		});
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
		let from_server_mechanism = new_transmission_mechanism(TransmissionMechanismBuilderArgument{name:&from_server_mechanism,virtual_channels: injection_buffers,..transmission_builder_argument});
		let transmission_port_status:Vec<Box<dyn StatusAtEmissor>> = (0..input_ports).map(|p|
			if let (Location::ServerPort(_server),_link_class)=topology.neighbour(router_index,p)
			{
				to_server_mechanism.new_status_at_emissor()
			}
			else
			{
				transmission_mechanism.new_status_at_emissor()
			}
		).collect();
		let reception_port_space = (0..input_ports).map(|p|
			if let (Location::ServerPort(_server),_link_class)=topology.neighbour(router_index,p)
			{
				from_server_mechanism.new_space_at_receptor()
			}
			else
			{
				transmission_mechanism.new_space_at_receptor()
			}
		).collect();
		let output_buffers= if output_buffer_size==0 {
			panic!("output_buffer_size must be greater than 0");
		} else {
			(0..input_ports).map(|_|
				(0..virtual_channels).map(|_|AugmentedBuffer::new()).collect()
			).collect()
		};
		let output_buffer_phits_traversing_crossbar = vec![ vec![ 0 ; virtual_channels ] ; input_ports ];

		let buffer_speed_metric = if let Some(time_segment) = time_segment_metric_buffer_rate
		{
			Some(
				(0..input_ports).map(|_|(0..virtual_channels).map(|_|TimeSegmentMetric::new(time_segment)).collect()).collect()
			)
		}else{
			None
		};
		let r=Rc::new(RefCell::new(InputOutput{
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
//			output_priorize_lowest_label,
			neglect_busy_output,
			buffer_size,
			crossbar_delay,
			transmission_port_status,
			reception_port_space,
			from_server_mechanism,
			output_buffer_size,
			output_buffers,
			output_buffer_phits_traversing_crossbar,
			output_schedulers: vec![],
			selected_input,
			selected_output,
			time_at_input_head,
			output_arbiter: OutputArbiter::Token{port_token: vec![0;input_ports]},
			maximum_packet_size,
			crossbar_frequency_divisor,
			buffer_speed_metric,
			crossbar_allocator: allocator,
			statistics_begin_cycle: 0,
			statistics_output_buffer_occupation_per_vc: vec![0f64;virtual_channels],
			statistics_reception_space_occupation_per_vc: vec![0f64;virtual_channels],
		}));
		//r.borrow_mut().self_rc=r.downgrade();
		r.borrow_mut().self_rc=Rc::<_>::downgrade(&r);
		r
	}
}

impl InputOutput
{
	///Whether a phit in an input buffer can advance.
	///bubble_in_use should be true only for leading phits that require the additional space.
	fn can_phit_advance(&self, phit:&Rc<Phit>, exit_port:usize, exit_vc:usize, bubble_in_use:bool)->bool
	{
		let available_internal_space = self.output_buffer_size-self.output_buffers[exit_port][exit_vc].len() - self.output_buffer_phits_traversing_crossbar[exit_port][exit_vc];
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


impl Eventful for InputOutput
{
	///main routine of the router. Do all things that must be done in a cycle, if any.
	fn process(&mut self, simulation:&SimulationShared, mutable:&mut SimulationMut) -> Vec<EventGeneration>
	{
		if self.output_schedulers.is_empty()
		{
			self.output_schedulers = (0..self.output_buffers.len()).map(|exit_port|{
				let (_location,link_class)=simulation.network.topology.neighbour(self.router_index,exit_port);
				let link = simulation.link_classes[link_class].clone();
				internal::TryLinkTraversalArgument{
					router:self,
					exit_port,
					link,
				}.into()
			}).collect();
		}
		let mut cycles_span = 1;//cycles since last checked
		if let Some(ref last)=self.last_process_at_cycle
		{
			cycles_span = simulation.cycle - *last;
			if *last >= simulation.cycle
			{
				panic!("Trying to process at cycle {} a router::InputOutput already processed at {}",simulation.cycle,last);
			}
			//if *last +1 < simulation.cycle
			//{
			//	println!("INFO: {} cycles since last processing router {}, cycle={}",simulation.cycle-*last,self.router_index,simulation.cycle);
			//}
		}
		//if cycles_span>=2
		//{
		//	println!("Processing router {index} at cycle {cycle} span={cycles_span}.",cycle=simulation.cycle,index=self.router_index);
		//}
		self.last_process_at_cycle = Some(simulation.cycle);
		assert!((simulation.cycle%self.crossbar_frequency_divisor) == 0, "Processing InputOutput router at a cycle ({cycle}) not multiple of crossbar_frequency_divisor ({divisor}). {cycle}%{divisor}={remainder}", cycle=simulation.cycle,divisor=self.crossbar_frequency_divisor,remainder=simulation.cycle%self.crossbar_frequency_divisor);
		let mut request:Vec<VCARequest>=vec![];
		let topology = simulation.network.topology.as_ref();
		
		let amount_virtual_channels=self.num_virtual_channels();
		//-- gather cycle statistics
		for (index, port_space) in self.reception_port_space.iter().enumerate()
		{
			for vc in 0..self.transmission_port_status[index].num_virtual_channels()//amount_virtual_channels
			{
				self.statistics_reception_space_occupation_per_vc[vc]+=(port_space.occupied_dedicated_space(vc).unwrap_or(0)*cycles_span as usize) as f64 / self.reception_port_space.len() as f64;
			}
		}
		for output_port in self.output_buffers.iter()
		{
			for (vc,buffer) in output_port.iter().enumerate()
			{
				self.statistics_output_buffer_occupation_per_vc[vc]+=(buffer.len()*cycles_span as usize) as f64 / self.output_buffers.len() as f64;
			}
		}

		//-- Precompute whatever polcies ask for.
		let server_ports : Option<Vec<usize>> = if self.virtual_channel_policies.iter().any(|policy|policy.need_server_ports())
		{
			Some((0..topology.ports(self.router_index)).filter(|&p|
				if let (Location::ServerPort(_server),_link_class)=topology.neighbour(self.router_index,p)
				{
					true
				}
				else
				{
					false
				}
			).collect())
		}
		else
		{
			None
		};
		let busy_ports:Vec<bool> = self.transmission_port_status.iter().enumerate().map(|(port,ref _status)|{
			let mut is_busy = false;
			for vc in 0..amount_virtual_channels
			{
				if let Some((selected_port,selected_virtual_channel))=self.selected_input[port][vc]
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
			Some(self.transmission_port_status.iter().map(|ref p|
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
			Some(self.transmission_port_status.iter().map(|ref p|{
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
		let port_occupied_output_space:Option<Vec<usize>> =
		{
			Some(self.output_buffers.iter().map(|p|
				p.iter().map(|b|b.len()).sum()
			).collect())
		};
		let port_available_output_space:Option<Vec<usize>> = 
		{
			Some(self.output_buffers.iter().map(|p|
				p.iter().map(|b|self.output_buffer_size - b.len()).sum()
			).collect())
		};
		let virtual_channel_occupied_output_space:Option<Vec<Vec<usize>>> =
		{
			Some(self.output_buffers.iter().map(|p|
				p.iter().map(|b|b.len()).collect()
			).collect())
		};
		let virtual_channel_available_output_space:Option<Vec<Vec<usize>>> =
		{
			Some(self.output_buffers.iter().map(|p|
				p.iter().map(|b|self.output_buffer_size-b.len()).collect()
			).collect())
		};

		//-- Routing and requests.
		let mut undecided_channels=0;//just as indicator if the router has pending work.
		let mut moved_input_phits=0;//another indicator of pending work.
		//Iterate over the reception space to find phits that request to advance.
		for entry_port in 0..self.reception_port_space.len()
		{
			for phit in self.reception_port_space[entry_port].front_iter()
			{
				let entry_vc={
					phit.virtual_channel.borrow().expect("it should have an associated virtual channel")
				};
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
							if good_ports.len()==0
							{
								break;//No need to check other policies.
							}
						}
						if good_ports.len()==0
						{
							self.time_at_input_head[entry_port][entry_vc]+=1;
							// if self.time_at_input_head[entry_port][entry_vc] > 25000
							// {
							// 	panic!("There are no choices for packet {:?} entry_port={} entry_vc={} in router {} towards server {} after policies.",phit.packet,entry_port,entry_vc,self.router_index,target_server);
							// }
							continue;//There is no available port satisfying the policies. Hopefully there will in the future.
						}
						//else if good_ports.len()>=2
						//{
						//	panic!("You need a VirtualChannelPolicy able to select a single (port,vc).");
						//}
						//simulation.routing.performed_request(&good_ports[0],&phit.packet.routing_info,simulation.network.topology.as_ref(),self.router_index,target_server,amount_virtual_channels,&mutable.rng);
						//match good_ports[0]
						//{
						//	CandidateEgress{port,virtual_channel,label,estimated_remaining_hops:_,..}=>(port,virtual_channel,label),
						//}
						for candidate in good_ports
						{
							simulation.routing.performed_request(&candidate,&phit.packet.routing_info,simulation.network.topology.as_ref(),self.router_index,target_router,Some(target_server),amount_virtual_channels,&mut mutable.rng);
							let CandidateEgress{port:requested_port,virtual_channel:requested_vc,label,..} = candidate;
//							if self.selected_input[requested_port][requested_vc].is_none()
//							{
								request.push( VCARequest{entry_port,entry_vc,requested_port,requested_vc,label});
//							}
						}
					},
					Some((_port,_vc)) => (),//(port,vc,0),//FIXME: perhaps 0 changes into None?
				};
				//FIXME: this should not call known_available_space_for_virtual_channel
				//In wormhole we may have a selected output but be unable to advance, but it is not clear whether makes any difference.
				/*let credits= self
					.transmission_port_status[requested_port]
					.known_available_space_for_virtual_channel(requested_vc)
					.expect("no available space known");
				//println!("entry_port={} virtual_channel={} credits={}",entry_port,entry_vc,credits);
				if credits>0
				{
					match self.selected_input[requested_port][requested_vc]
					{
						Some(_) => (),
						None => request.push( VCARequest{entry_port,entry_vc,requested_port,requested_vc,label} ),
					};
				}*/
				self.time_at_input_head[entry_port][entry_vc]+=1;
			}
		}

		let captured_intransit_priority=self.intransit_priority;
		// Check if the allocator supports intransit priority.
		if captured_intransit_priority {
			// If the allocator supports intransit priority
			if !self.crossbar_allocator.support_intransit_priority() {
				panic!("Current crossbar allocator does not support intransit priority option");
			}

			//to each request in request, set label to 0 if it is a transit request.
			request = request.into_iter().map(|mut req|{
				if let (Location::RouterPort { .. },_) = simulation.network.topology.neighbour(self.router_index,req.entry_port)
				{
					req.label = 0;
				}
				req
			}).collect();
		}

		// Add all the requests to the allocator.
		request.iter_mut().for_each(|pr| {
			self.crossbar_allocator.add_request(pr.to_allocator_request(amount_virtual_channels));
		});

		// Perform the allocation
		let mut requests_granted : Vec<VCARequest> = Vec::new();
		for gr in self.crossbar_allocator.perform_allocation(&mut mutable.rng) {
			// convert from allocator Request to VCARequest
			requests_granted.push(gr.to_port_request(amount_virtual_channels));
		}
	
		let request_it = requests_granted.into_iter();

		//Complete the arbitration of the requests by writing the selected_input of the output virtual ports.
		//let request=request_sequence.concat();
		for VCARequest{entry_port,entry_vc,requested_port,requested_vc,..} in request_it
		{
			self.selected_input[requested_port][requested_vc]=Some((entry_port,entry_vc));
			self.selected_output[entry_port][entry_vc]=Some((requested_port,requested_vc));
		}

		//-- For each output port decide which input actually uses it this cycle.
		let mut events=vec![];
		for exit_port in 0..self.transmission_port_status.len()
		{
			let nvc=amount_virtual_channels;
			for exit_vc in 0..nvc
			{
				if let Some((entry_port,entry_vc))=self.selected_input[exit_port][exit_vc]
				{
					//-- Move phits into the internal output space
					//Note that it is possible when flit_size<packet_size for the packet to not be in that buffer. The output arbiter can decide to advance other virtual channel.
					if let Ok((phit,ack_message)) = self.reception_port_space[entry_port].extract(entry_vc)
					{
						// For the check with crossbar delay look into PhitToOutput::process.
						if self.output_buffers[exit_port][exit_vc].len()>=self.output_buffer_size
						{
							panic!("Trying to move into a full output buffer.");
						}
						moved_input_phits+=1;
						self.time_at_input_head[entry_port][entry_vc]=0;
						*phit.virtual_channel.borrow_mut()=Some(exit_vc);
						if let Some(message)=ack_message
						{
							// If the crossbar operates at higher frequency (aka internal speedup) then it would send acks at greater rate than allowed.
							// We allow sending several events in the same cycle of the link. Acks should have few bits and be possible to be aggregated.
							let (previous_location,previous_link_class)=simulation.network.topology.neighbour(self.router_index,entry_port);
							let event = Event::Acknowledge{location:previous_location,message};
							events.push(simulation.schedule_link_arrival( previous_link_class, event ));
						}
						if phit.is_end()
						{
							self.selected_input[exit_port][exit_vc]=None;
							self.selected_output[entry_port][entry_vc]=None;
						}
						else
						{
							self.selected_output[entry_port][entry_vc]=Some((exit_port,exit_vc));
						}
						if self.crossbar_delay==0 {
							self.output_buffers[exit_port][exit_vc].push(phit,(entry_port,entry_vc));
							let mut output_scheduler = self.output_schedulers[exit_port].borrow_mut();
							if let Some(event) = output_scheduler.schedule(simulation.cycle,0) {
								events.push(event);
							}
						} else {
							let event = Rc::<RefCell<internal::PhitToOutput>>::from(internal::PhitToOutputArgument{
								//router: self.self_rc.upgrade().unwrap(),
								router: self,
								exit_port,
								exit_vc,
								entry_port,
								entry_vc,
								phit,
							});
							events.push(EventGeneration{
								delay: self.crossbar_delay,
								position:CyclePosition::Begin,
								event: Event::Generic(event),
							});
						}
					}
					else
					{
						if self.flit_size>1
						{
							//XXX We seem to easily reach this region when using different frequencies.
							//We would like to panic if phit.packet.size<=flit_size, but we do not have the phit accesible.
							//println!("WARNING: There were no phit at the selected_input[{}][{}]=({},{}) of the router {}.",exit_port,exit_vc,entry_port,entry_vc,self.router_index);
						}
					}
				}
			}
		}
		self.next_events.pop();//remove the event that was served.
		//TODO: what to do with probabilistic requests???
		//if undecided_channels>0 || moved_phits>0 || events.len()>0 || request.len()>0
		//if undecided_channels>0 || moved_phits>0 || events.len()>0
		let recheck_crossbar = undecided_channels>0 || moved_input_phits>0 || request.len()>0;//Needs to check the crossbar in its next slot.
		if recheck_crossbar {
			let next_delay = event::round_to_multiple(simulation.cycle+1,self.crossbar_frequency_divisor) - simulation.cycle;
			if let Some(event) = self.schedule(simulation.cycle,next_delay)
			{
				events.push(event);
			}
		}
		events
	}
	fn as_eventful(&self)->Weak<RefCell<dyn Eventful>>
	{
		self.self_rc.clone()
	}
	/**
	We schedule in cycles multiple of the `crossbar_frequency_divisor`.
	Note the outputs of the router are instead scheduled by `TryLinkTraversal::schedule`.
	**/
	fn schedule(&mut self, current_cycle:Time, delay:Time) -> Option<EventGeneration>
	{
		let target = current_cycle+delay;
		let target = event::round_to_multiple(target,self.crossbar_frequency_divisor);
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
// /*
//  Value gathered at some time
//  */
// #[derive(Clone)]
// struct TimeValue {
// 	value: f64,
// 	time: Time,
// }

/*
 Value gathered at a period of time
 */
#[derive(Clone,Debug)]
struct TimeSegmentValue {
	value: f64,
	#[allow(dead_code)]//It seems clear that is going to be used in the future.
	begin_time: Time,
	end_time: Time,
}
/**
 Metric to be measured in a time segment
 in_use_metric: Current value in use for router operations. It's gathered from the last time segment
 measure_metric: Value being measured at the time
 time_segment: Time segment to measure the metric
 **/
#[derive(Debug)]
struct TimeSegmentMetric{
	in_use_metric: TimeSegmentValue,
	measure_metric: TimeSegmentValue,
	time_segment: usize,
}
impl TimeSegmentMetric{
	fn new(time_segment:usize)->TimeSegmentMetric{
		TimeSegmentMetric{
			in_use_metric: TimeSegmentValue {value:0.0,begin_time:0, end_time:0},
			measure_metric: TimeSegmentValue {value:0.0,begin_time:0, end_time: time_segment as Time },
			time_segment,
		}
	}
	fn add_measure(&mut self, value: f64, time: Time){
		self.check_refresh(time);
		self.measure_metric.value += value;
	}
	fn check_refresh(&mut self, time: Time){
		if time >= self.measure_metric.end_time as u64{
			let offset = (self.time_segment * (time as usize/self.time_segment)) as u64;
			self.in_use_metric = self.measure_metric.clone();
			self.measure_metric = TimeSegmentValue{value:0.0, begin_time: offset, end_time: offset + self.time_segment as u64};
		}
	}
}


impl Quantifiable for InputOutput
{
	fn total_memory(&self) -> usize
	{
		//FIXME: redo
		//return size_of::<InputOutput<TM>>() + self.virtual_ports.total_memory() + self.port_token.total_memory();
		return size_of::<InputOutput>();
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

/// Some things private to InputOutput we want to have clearly separated.
mod internal
{
	use super::*;
	/**
	Insert a phit into an output queue. Created when the phits are extracted from the input at a crossbar time slot.
	**/
	pub struct PhitToOutput
	{
		self_rc: Weak<RefCell<PhitToOutput>>,
		router: Rc<RefCell<InputOutput>>,
		exit_port: usize,
		exit_vc: usize,
		entry_port: usize,
		entry_vc: usize,
		phit: Rc<Phit>,
	}
	pub struct PhitToOutputArgument<'a>
	{
		pub router: &'a mut InputOutput,
		pub exit_port: usize,
		pub exit_vc: usize,
		pub entry_port: usize,
		pub entry_vc: usize,
		pub phit: Rc<Phit>,
	}
	impl<'a> From<PhitToOutputArgument<'a>> for Rc<RefCell<PhitToOutput>>
	{
		fn from(arg:PhitToOutputArgument) -> Rc<RefCell<PhitToOutput>>
		{
			arg.router.output_buffer_phits_traversing_crossbar[arg.exit_port][arg.exit_vc]+=1;
			let event = Rc::new(RefCell::new(PhitToOutput{
				self_rc: Weak::new(),
				//router: arg.router,
				router: arg.router.self_rc.upgrade().unwrap(),
				exit_port: arg.exit_port,
				exit_vc: arg.exit_vc,
				entry_port: arg.entry_port,
				entry_vc: arg.entry_vc,
				phit: arg.phit,
			}));
			event.borrow_mut().self_rc=Rc::<_>::downgrade(&event);
			event
		}
	}
	impl Eventful for PhitToOutput
	{
		fn process(&mut self, simulation:&SimulationShared, _mutable:&mut SimulationMut) -> Vec<EventGeneration>
		{
			let mut router = self.router.borrow_mut();
			if router.output_buffers[self.exit_port][self.exit_vc].len()>=router.output_buffer_size
			{
				panic!("(PhitToOutput) Trying to move into a full output buffer.");
			}
			router.output_buffer_phits_traversing_crossbar[self.exit_port][self.exit_vc]-=1;
			router.output_buffers[self.exit_port][self.exit_vc].push(self.phit.clone(),(self.entry_port,self.entry_vc));
			let mut output_scheduler = router.output_schedulers[self.exit_port].borrow_mut();
			if let Some(event) = output_scheduler.schedule(simulation.cycle,0) {
				vec![event]
			} else {
				vec![]
			}
		}
		///Extract the eventful from the implementing class. Required since `as Rc<RefCell<Eventful>>` does not work.
		fn as_eventful(&self)->Weak<RefCell<dyn Eventful>>
		{
			self.self_rc.clone()
		}
	}
	/**
	Process an output port and, if possible, extract some phit to send through the link.
	Scheduled at the link frequency.
	**/
	use crate::LinkClass;
	pub struct TryLinkTraversal
	{
		self_rc: Weak<RefCell<TryLinkTraversal>>,
		router: Rc<RefCell<InputOutput>>,
		exit_port: usize,
		link:LinkClass,
		amount_virtual_channels: usize,
		pending_event:bool,
	}
	pub struct TryLinkTraversalArgument<'a>
	{
		pub router: &'a mut InputOutput,
		pub exit_port: usize,
		pub link: LinkClass
	}
	impl<'a> From<TryLinkTraversalArgument<'a>> for Rc<RefCell<TryLinkTraversal>>
	{
		fn from(arg:TryLinkTraversalArgument) -> Rc<RefCell<TryLinkTraversal>>
		{
			let amount_virtual_channels = arg.router.num_virtual_channels();
			let this = Rc::new(RefCell::new(TryLinkTraversal{
				self_rc: Weak::new(),
				router: arg.router.self_rc.upgrade().unwrap(),
				exit_port: arg.exit_port,
				link: arg.link,
				amount_virtual_channels,
				pending_event:false,
			}));
			this.borrow_mut().self_rc=Rc::<_>::downgrade(&this);
			this
		}
	}
	impl Eventful for TryLinkTraversal
	{
		fn process(&mut self, simulation:&SimulationShared, mutable:&mut SimulationMut) -> Vec<EventGeneration>
		{
			let mut events=vec![];
			let mut router = self.router.borrow_mut();
			let nvc= self.amount_virtual_channels;
			//Gather the list of all vc that can advance
			let mut cand=Vec::with_capacity(nvc);
			let mut cand_in_transit=false;
//			let mut undo_selected_input=Vec::with_capacity(nvc);
			//let is_link_cycle = simulation.is_link_cycle(link_class);
			for exit_vc in 0..nvc
			{
				//Candidates when using output ports.
				if let Some( (phit,(entry_port,_entry_vc))) = router.output_buffers[self.exit_port][exit_vc].front()
				{
					let bubble_in_use= router.bubble && phit.is_begin() && simulation.network.topology.is_direction_change(router.router_index,entry_port,self.exit_port);
					let status=&router.transmission_port_status[self.exit_port];
					let can_transmit = if bubble_in_use
					{
						//router.transmission_port_status[self.exit_port].can_transmit_whole_packet(&phit,exit_vc)
						if let Some(space)=status.known_available_space_for_virtual_channel(exit_vc)
						{
							status.can_transmit(&phit,exit_vc) && space>= phit.packet.size + router.maximum_packet_size
						}
						else
						{
							panic!("InputOutput router requires knowledge of available space to apply bubble.");
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
						if 0<phit.index && phit.index<router.flit_size
						{
							panic!("cannot transmit phit (index={}) but it should (flit_size={})",phit.index,router.flit_size);
						}
					}
				}
			}
			//for selected_virtual_channel in 0..nvc
			if !cand.is_empty()
			{
				//Then select one of the vc candidates (either in input or output buffer) to actually use the physical port.
				let selected_virtual_channel = match router.output_arbiter
				{
					OutputArbiter::Random=> cand[mutable.rng.gen_range(0..cand.len())],
					OutputArbiter::Token{ref mut port_token}=>
					{
						//Or by tokens as in fsin
						//let nvc=router.virtual_ports[self.exit_port].len() as i64;
						let nvc= self.amount_virtual_channels as i64;
						let token= port_token[self.exit_port] as i64;
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
						port_token[self.exit_port]=best;
						best
					},
				};
				//move phits around.
				let (phit,original_port) =
				{
					//If we get the phit from an output buffer there is little to do.
					let (phit,(entry_port,_entry_vc))=router.output_buffers[self.exit_port][selected_virtual_channel].pop().expect("incorrect selected_input");
					(phit,entry_port)
				};
				//Send the phit to the other link endpoint.
				let (new_location,_link_class)=simulation.network.topology.neighbour(router.router_index,self.exit_port);
				//let link = &simulation.link_classes[link_class];
				events.push(EventGeneration{
					delay: self.link.delay,
					position:CyclePosition::Begin,
					event:Event::PhitToLocation{
						phit: phit.clone(),
						previous: Location::RouterPort{
							router_index: router.router_index,
							router_port: original_port,
						},
						new: new_location,
					},
				});
				//next_delay = Some(next_delay.unwrap_or(link.frequency_divisor).min(link.frequency_divisor));
				router.transmission_port_status[self.exit_port].notify_outcoming_phit(selected_virtual_channel,simulation.cycle);
				if phit.is_end()
				{
					if let OutputArbiter::Token{ref mut port_token}=router.output_arbiter
					{
						port_token[self.exit_port]=(port_token[self.exit_port]+1)% self.amount_virtual_channels;
					}
				}
			}
			drop(router);//to be able to mutate self
			self.pending_event = false;
			// XXX we should avoid to reschedule when it is not necessary.
			// Are we sure that if we have not being able to advance and nothing changes then we are indefinitely idle?
			// It is important that the acks received by the router may trigger the scheduling.
			if !events.is_empty()
			{
				if let Some(event) = self.schedule(simulation.cycle,1)
				{
					events.push(event);
				}
			}
			events
		}
		fn as_eventful(&self)->Weak<RefCell<dyn Eventful>>
		{
			self.self_rc.clone()
		}
		fn schedule(&mut self, current_cycle:Time, delay:Time) -> Option<EventGeneration>
		{
			if !self.pending_event {
				self.pending_event=true;
				let event = Event::Generic(self.as_eventful().upgrade().expect("missing component"));
				let target = current_cycle+delay;
				let target = event::round_to_multiple(target,self.link.frequency_divisor);
				let delay = target - current_cycle;
				Some(EventGeneration{
					delay,
					position: CyclePosition::End,
					event,
				})
			} else {
				None
			}
		}
	}
}


// #[cfg(test)]
// mod tests {
// 	use procfs::sys::vm::DropCache::Default;
// 	use crate::{Message, Packet, PacketRef, Plugs};
// 	use crate::router::new_router;
// 	use super::*;
//
// 	fn create_input_output_router(router_index: usize, topology: Box<dyn Topology>, mut rng: &StdRng ) -> InputOutput
// 	{
// 		//let router_index= 0;
// 		let plugs = Plugs::default();
//
// 		let maximum_packet_size = 16;
// 		let general_frequency_divisor = 1;
//
// 		//let mut output_buffer_size=None;
// 		//let mut allocator_value=None;
// 		let mut transmission_mechanism=None;
// 		let mut to_server_mechanism=None;
// 		let mut from_server_mechanism=None;
// 		let mut crossbar_delay: Time =0;
// 		let mut neglect_busy_output = false;
// 		let mut crossbar_frequency_divisor = general_frequency_divisor;
//
//
// 		//let sides=sides.expect("There were no sides");
// 		let virtual_channels= 4; //virtual_channels.expect("There were no virtual_channels");
// 		let virtual_channel_policies= vec![]; // virtual_channel_policies.expect("There were no virtual_channel_policies");
// 		//let routing=routing.expect("There were no routing");
// 		let buffer_size= 64; //buffer_size.expect("There were no buffer_size");
// 		let output_buffer_size=32; //output_buffer_size.expect("There were no output_buffer_size");
// 		let bubble= false; //bubble.expect("There were no bubble");
// 		let flit_size=16; //flit_size.expect("There were no flit_size");
// 		let intransit_priority= false; //intransit_priority.expect("There were no intransit_priority");
// 		let allow_request_busy_port= true; //allow_request_busy_port.expect("There were no allow_request_busy_port");
// //		let output_priorize_lowest_label=output_priorize_lowest_label.expect("There were no output_priorize_lowest_label");
// 		let input_ports= 2; //topology.ports(router_index);
//
// 		use rand::SeedableRng;
// 		let mut rng_allo =StdRng::seed_from_u64(10u64);
// 		let allo_cv = ConfigurationValue::Object("Random".to_string(),vec![("seed".to_string(),ConfigurationValue::Number(1f64))]);
// 		let allocator = new_allocator(AllocatorBuilderArgument{
// 			cv: &allo_cv,//&allocator_value.expect("There were no allocator"),
// 			num_clients:input_ports * virtual_channels,
// 			num_resources:input_ports * virtual_channels,
// 			plugs: &plugs,
// 			rng: &mut rng_allo,
// 		});
//
// 		let selected_input=(0..input_ports).map(|_|
// 			(0..virtual_channels).map(|_|None).collect()
// 		).collect();
//
// 		let selected_output=(0..input_ports).map(|_|
// 			(0..virtual_channels).map(|_|None).collect()
// 		).collect();
//
// 		let time_at_input_head=(0..input_ports).map(|_|
// 			(0..virtual_channels).map(|_|0).collect()
// 		).collect();
//
// 		let transmission_mechanism = transmission_mechanism.unwrap_or_else(||"SimpleVirtualChannels".to_string());
// 		//let from_server_mechanism = from_server_mechanism.unwrap_or_else(||"TransmissionFromServer".to_string());
// 		let from_server_mechanism = from_server_mechanism.unwrap_or_else(||"SimpleVirtualChannels".to_string());
// 		let to_server_mechanism = to_server_mechanism.unwrap_or_else(||"TransmissionToServer".to_string());
// 		//let transmission_mechanism = super::SimpleVirtualChannels::new(virtual_channels,buffer_size,flit_size);
// 		let transmission_builder_argument = TransmissionMechanismBuilderArgument{name:"",virtual_channels,buffer_size,size_to_send:flit_size};
// 		let transmission_mechanism = new_transmission_mechanism(TransmissionMechanismBuilderArgument{name:&transmission_mechanism,..transmission_builder_argument});
// 		let to_server_mechanism = new_transmission_mechanism(TransmissionMechanismBuilderArgument{name:&to_server_mechanism,..transmission_builder_argument});
// 		//let from_server_mechanism = TransmissionFromServer::new(virtual_channels,buffer_size,flit_size);
// 		let from_server_mechanism = new_transmission_mechanism(TransmissionMechanismBuilderArgument{name:&from_server_mechanism,..transmission_builder_argument});
//
// 		let transmission_port_status:Vec<Box<dyn StatusAtEmissor>> = (0..input_ports).map(|p|
// 			if let (Location::ServerPort(_server),_link_class)=topology.neighbour(router_index,p)
// 			{
// 				to_server_mechanism.new_status_at_emissor()
// 			}
// 			else
// 			{
// 				transmission_mechanism.new_status_at_emissor()
// 			}
// 		).collect();
//
// 		let reception_port_space = (0..input_ports).map(|p|
// 			if let (Location::ServerPort(_server),_link_class)=topology.neighbour(router_index,p)
// 			{
// 				from_server_mechanism.new_space_at_receptor()
// 			}
// 			else
// 			{
// 				transmission_mechanism.new_space_at_receptor()
// 			}
// 		).collect();
//
// 		let output_buffers= if output_buffer_size==0 {
// 			panic!("output_buffer_size must be greater than 0");
// 		} else {
// 			(0..input_ports).map(|_|
// 				(0..virtual_channels).map(|_|AugmentedBuffer::new()).collect()
// 			).collect()
// 		};
// 		let output_buffer_phits_traversing_crossbar = vec![ vec![ 0 ; virtual_channels ] ; input_ports ];
//
// 		InputOutput{
//
// 			self_rc: Weak::new(),
// 			next_events: vec![],
// 			last_process_at_cycle: None,
// 			router_index,
// 			//routing,
// 			virtual_channel_policies,
// 			bubble,
// 			flit_size,
// 			intransit_priority,
// 			allow_request_busy_port,
// //			output_priorize_lowest_label,
// 			neglect_busy_output,
// 			buffer_size,
// 			crossbar_delay,
// 			transmission_port_status,
// 			reception_port_space,
// 			from_server_mechanism,
// 			output_buffer_size,
// 			output_buffers,
// 			output_buffer_phits_traversing_crossbar,
// 			output_schedulers: vec![],
// 			selected_input,
// 			selected_output,
// 			time_at_input_head,
// 			output_arbiter: OutputArbiter::Token{port_token: vec![0;input_ports]},
// 			maximum_packet_size,
// 			crossbar_frequency_divisor,
// 			crossbar_allocator: allocator,
// 			statistics_begin_cycle: 0,
// 			statistics_output_buffer_occupation_per_vc: vec![0f64;virtual_channels],
// 			statistics_reception_space_occupation_per_vc: vec![0f64;virtual_channels],
// 		}
// 	}
//
// 	#[test]
// 	fn input_output_test()
// 	{
// 		use crate::topology::{new_topology,TopologyBuilderArgument};
// 		use crate::RoutingInfo;
// 		use rand::SeedableRng;
//
// 		let mut rng=StdRng::seed_from_u64(10u64);
// 		let plugs = Plugs::default();
// 		//let cv = ConfigurationValue::Object("FixedRandom".to_string(),vec![("allow_self".to_string(),ConfigurationValue::True)]);
// 		// TODO: topology::dummy?
// 		let topo_cv = ConfigurationValue::Object("Hamming".to_string(),vec![("sides".to_string(),ConfigurationValue::Array(vec![])), ("servers_per_router".to_string(),ConfigurationValue::Number(1.0))]);
// 		let topology = new_topology(TopologyBuilderArgument{cv:&topo_cv,plugs:&plugs,rng:&mut rng});
//
// 		let mut router = Rc::new(RefCell::new( create_input_output_router(0,topology, &mut rng) ));
// 		router.borrow_mut().self_rc=Rc::<_>::downgrade(&router);
// 		let time = 1u64;
//
//
// 		let m = Message{
// 			origin: 0,
// 			destination: 0,
// 			size:16,
// 			creation_cycle: time,
// 		};
//
// 		let p = Packet{
// 			size:16,
// 			routing_info: RefCell::new(RoutingInfo::new()),
// 			message: Rc::from(m), //Message{},
// 			index:0,
// 			cycle_into_network:RefCell::new(0),
// 			extra: RefCell::new(None),
// 		}.into_ref();
//
// 		let phit = Rc::new(Phit{
//             packet: p,
//             index: 0,
//             virtual_channel: RefCell::new(None),
//         });
//
// 		router.borrow_mut().insert(time, phit, 0, &mut rng);
//
// 		let ss = SimulationShared{
// 			cycle:0,
// 			network: Network{
// 				topology,
// 				routers,
// 				servers,
// 			},
// 			traffic,
// 			routing,
// 			link_classes,
// 			maximum_packet_size,
// 			general_frequency_divisor,
// 		};
//
// 		let sm = SimulationMut{
// 			rng,
// 		};
//
// 		router.borrow_mut().process(ss, sm);
//
// 	}
// }


