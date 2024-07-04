/*!

Module encapsulating the statistics about several measures.

The main measures are documented as fields of StatisticMeasurement. The names in the result files are defined inside `Simulation::write_result`.

The values always written into a local.result are:
* `cycle` is the last simulated cycle.
* `injected_load` is the average load injected from the servers into the network during the main sampled period.
* `accepted_load` is the average load consumed by the servers from the network during the main sampled period.
* `average_message_delay` is the average delay of messages consumed during the main sampled period. The delay of a message counts from the cycle in which
the message was created until the cycle in its consumption was completed. Note the creation time may be before the main sampled period started.
* `average_packet_network_delay` is the average network delay of packets consumed during the main sampled period. This network delay only includes the time since the packet was injected into the network until its consumption. This is, it explicitly ignores the span from creation until injection.
* `server_generation_jain_index` is the Jain index associated to the load injected by the servers. This is a fairness measure and it will be close to 1 when all servers are generating a similar load, regardless of its magnitude.
* `server_consumption_jain_index` is the Jain index associated to the load consumed by the servers. This is a fairness measure and it will be close to 1 when all servers are consuming a similar load, regardless of its magnitude.
* `average_packet_hops` is the average number of router-to-router hops traveled by network packets during the main sampled period.
* `total_packet_per_hop_count` is an array with the total number of consumed packets indexes by the number of hops they travelled.
* `average_link_utilization` is the average utilization of links during the main sampled period. This is 1 when each link is being used to transmit a phit every cycle. This is a measure of well-balanced load.
* `maximum_link_utilization` is the average utilization of the most used link during the main sampled period. This is 1 when some link has being used to transmit a phit every cycle. If this value does not reach 1 it may indicate a deficiency in the router.
* `server_average_cycle_last_created_phit` is the average of the timestamps in which the servers have created their last phit. This measure is intended for traffics that have a clear ending.
* `server_average_cycle_last_consumed_message` is the average of the timestamps in which the servers have last consumed a phit. This measure is intended for traffics that have a clear ending.
* `server_average_missed_generations` counts the average of times a server has skipped generating a message because its internal queue is full. Under some assumptions a greater than 0 value means some flows have infinite latency. It may also mean that `server_queue_size` is not large enough.
* `servers_with_missed_generations` counts the number of severs that have missed some generations. Under some assumptions this is couting the number of flows with infinite latency.
* `virtual_channel_usage` is an array with the link utilization indexed by the virtual channel. This is, when a phit is transmitted by a link requesting a virtual channel `vc`, a `+1` is tracked into the index `vc`.
* `git_id` has an id of the CAMINOS binary, which is meaningful when building from a git repository.
* `version_number` has the CAMINOS version as read from the Cargo.toml.

*/


use std::cmp;
use std::collections::HashMap;
use std::path::Path;
use std::convert::TryInto;

use crate::{Quantifiable,Packet,Phit,Network,Topology,ConfigurationValue,Expr,Time};
use crate::config;
use crate::traffic::TaskTrafficState;

#[derive(Clone,Quantifiable)]
pub struct ServerStatistics
{
	pub current_measurement: ServerMeasurement,
	///The last cycle in which this server created a phit and sent it to a router. Or 0
	pub cycle_last_created_phit: Time,
	///The last cycle in that the last phit of a message has been consumed by this server. Or 0.
	pub cycle_last_consumed_message: Time,
	///If non-zero then creates statistics for intervals of the given number of cycles.
	pub temporal_step: Time,
	///The periodic measurements requested by non-zero statistics_temporal_step.
	pub temporal_statistics: Vec<ServerMeasurement>,
}

#[derive(Clone,Default,Quantifiable)]
pub struct ServerMeasurement
{
	///The number of the first cycle included in the statistics.
	pub begin_cycle: Time,
	pub created_phits: usize,
	pub consumed_phits: usize,
	pub consumed_messages: usize,
	pub total_message_delay: Time,
	///Number of times the traffic returned true from `should_generate`, but it could not be stored.
	pub missed_generations: usize,
}

impl ServerStatistics
{
	pub fn new(temporal_step:Time)->ServerStatistics
	{
		ServerStatistics{
			current_measurement: ServerMeasurement::default(),
			cycle_last_created_phit: 0,
			cycle_last_consumed_message: 0,
			temporal_step,
			temporal_statistics: vec![],
		}
	}
	fn reset(&mut self, next_cycle: Time)
	{
		self.current_measurement=ServerMeasurement::default();
		self.current_measurement.begin_cycle=next_cycle;
	}
	/// Called each time a server consumes a phit.
	pub fn track_consumed_phit(&mut self, cycle:Time)
	{
		self.current_measurement.consumed_phits+=1;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.consumed_phits+=1;
		}
	}
	/// Called when a server consumes the last phit from a message.
	pub fn track_consumed_message(&mut self, cycle: Time)
	{
		self.cycle_last_consumed_message = cycle;
		self.current_measurement.consumed_messages+=1;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.consumed_messages+=1;
		}
	}
	/// Called each time the server creates a phit.
	pub fn track_created_phit(&mut self, cycle: Time)
	{
		self.current_measurement.created_phits+=1;
		self.cycle_last_created_phit = cycle;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.created_phits+=1;
		}
	}
	/// Called when a server consumes the last phit from a message.
	/// XXX: Perhaps this should be part of `track_consumed_message`.
	pub fn track_message_delay(&mut self, delay:Time, cycle: Time)
	{
		self.current_measurement.total_message_delay+= delay;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.total_message_delay+=delay;
		}
	}
	/// Called when the server should have generated a new message but it did not have space in queue.
	pub fn track_missed_generation(&mut self, cycle: Time)
	{
		self.current_measurement.missed_generations+=1;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.missed_generations+=1;
		}
	}
	pub fn current_temporal_measurement(&mut self, cycle: Time) -> Option<&mut ServerMeasurement>
	{
		if self.temporal_step>0
		{
			let index : usize = (cycle / self.temporal_step).try_into().unwrap();
			if self.temporal_statistics.len()<=index
			{
				self.temporal_statistics.resize_with(index+1,Default::default);
				self.temporal_statistics[index].begin_cycle = index as Time * self.temporal_step;
			}
			Some(&mut self.temporal_statistics[index])
		} else { None }
	}
}

#[derive(Clone,Quantifiable, Debug)]
pub struct TrafficStatistics
{
	///The number of tasks in the traffic
	pub tasks: usize,
	///The last cycle in which this server created a phit and sent it to a router. Or 0
	pub cycle_last_created_message: Time,
	///The last cycle in that the last phit of a message has been consumed by this server. Or 0.
	pub cycle_last_consumed_message: Time,
	///If non-zero then creates statistics for intervals of the given number of cycles.
	pub temporal_step: Time,
	///Current measurment for temporal statistics.
	pub current_measurement: TrafficMeasurement,
	///The periodic measurements requested by non-zero statistics_temporal_step.
	pub temporal_statistics: Vec<TrafficMeasurement>,
	/// The total number of messages created.
	pub total_created_messages: usize,
	/// The total number of phits created. Could be in Bytes also.
	pub total_created_phits: usize,
	/// The total number of messages consumed.
	pub total_consumed_messages: usize,
	/// The total number of phits consumed.
	pub total_consumed_phits: usize,
	/// The total delay of all messages.
	pub total_message_delay: Time,
	/// The total network delay of all packets.
	pub total_message_network_delay: Time,
	/// The statistics of other subtraffic.
	pub sub_traffic_statistics: Option<Vec<TrafficStatistics>>,
	/// Box size histogram
	pub box_size: usize,
	/// Messages histogram
	pub histogram_messages_delay: HashMap<usize, usize>,
	// /// Messages histogram network delay
	// pub histogram_messages_network_delay: HashMap<usize, usize>,
	/// Generating Tasks
	pub generating_tasks_histogram: HashMap<usize, Vec<usize>>,
	/// Waiting Tasks
	pub waiting_tasks_histogram: HashMap<usize, Vec<usize>>,
	/// FinishedGenerating Tasks
	pub finished_generating_tasks_histogram: HashMap<usize, Vec<usize>>,
	///Finished Tasks
	pub finished_tasks_histogram: HashMap<usize, Vec<usize>>,
	///WaitingData Tasks
	pub waiting_data_histogram: HashMap<usize, Vec<usize>>,
}

#[derive(Clone,Default,Quantifiable,Debug)]
pub struct TrafficMeasurement
{
	pub begin_cycle: Time,
	pub created_messages: usize,
	pub created_phits: usize,
	pub consumed_messages: usize,
	pub consumed_phits: usize,
	pub total_message_delay: Time,
}

impl TrafficStatistics
{
	pub fn new(tasks: usize, temporal_step:Time, box_size: usize, sub_traffic_statistics: Option<Vec<TrafficStatistics>>)-> TrafficStatistics
	{
		TrafficStatistics {
			tasks,
			current_measurement: TrafficMeasurement::default(),
			cycle_last_created_message: 0,
			cycle_last_consumed_message: 0,
			temporal_step,
			temporal_statistics: vec![],
			total_created_messages: 0,
			total_created_phits: 0,
			total_consumed_messages: 0,
			total_consumed_phits: 0,
			total_message_delay: 0,
			total_message_network_delay: 0,
			sub_traffic_statistics,
			box_size,
			histogram_messages_delay: HashMap::new(),
			// histogram_messages_network_delay: HashMap::new(),
			generating_tasks_histogram: HashMap::new(),
			waiting_tasks_histogram: HashMap::new(),
			finished_generating_tasks_histogram: HashMap::new(),
			finished_tasks_histogram: HashMap::new(),
			waiting_data_histogram: HashMap::new(),
		}
	}
	// fn reset(&mut self, next_cycle: Time)
	// {
	// 	self.current_measurement= TrafficMeasurement::default();
	// 	self.current_measurement.begin_cycle=next_cycle;
	// }

	/// Called when a task recieves a message.
	//	pub fn track_consumed_message(&mut self, cycle: Time, total_delay:Time, injection_delay:Time, size: usize, subtraffic: Option<usize>)
	pub fn track_consumed_message(&mut self, cycle: Time, total_delay:Time, size: usize, subtraffic: Option<usize>)
	{
		// if delay < 0
		// {
		// 	panic!("negative delay");
		// }
		// if total_delay < injection_delay
		// {
		// 	panic!("negative total delay");
		// }
		// let message_network_delay = total_delay - injection_delay;
		self.cycle_last_consumed_message = cycle;
		self.total_consumed_messages+=1;
		self.total_message_delay+=total_delay;
		// self.total_message_network_delay+= message_network_delay;
		self.total_consumed_phits+=size;

		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.consumed_messages+=1;
			m.consumed_phits+=size;
			m.total_message_delay+=total_delay;
		}

		self.histogram_messages_delay.entry(total_delay as usize/self.box_size).and_modify(|e| *e+=1).or_insert(1);
		// self.histogram_messages_network_delay.entry(message_network_delay as usize/self.box_size).and_modify(|e| *e+=1).or_insert(1);

		if let Some(subtraffic) = subtraffic
		{
			if let Some(sub) = self.sub_traffic_statistics.as_mut()
			{
				sub[subtraffic].track_consumed_message(cycle, total_delay, size, None);
			}else {
				panic!("Subtraffic statistics not initialized");
			}
		}
	}
	/// Called each time the traffic creates a message.
	pub fn track_created_message(&mut self, cycle: Time, size:usize, subtraffic: Option<usize>)
	{
		self.cycle_last_created_message = cycle;
		self.total_created_messages+=1;
		self.total_created_phits+=size;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.created_messages+=1;
			m.created_phits+=size;
		}
		if let Some(subtraffic) = subtraffic
		{
			if let Some(sub) = self.sub_traffic_statistics.as_mut()
			{
				sub[subtraffic].track_created_message(cycle,size,None);
			}else {
				panic!("Subtraffic statistics not initialized");
			}
		}

	}

	pub fn current_temporal_measurement(&mut self, cycle: Time) -> Option<&mut TrafficMeasurement>
	{
		if self.temporal_step>0
		{
			let index : usize = (cycle / self.temporal_step).try_into().unwrap();
			if self.temporal_statistics.len()<=index
			{
				self.temporal_statistics.resize_with(index+1,Default::default);
				self.temporal_statistics[index].begin_cycle = index as Time * self.temporal_step;
			}
			Some(&mut self.temporal_statistics[index])
		} else { None }
	}

	pub fn track_task_state(&mut self, task: usize, state: TaskTrafficState, cycle: Time, subtraffic: Option<usize>)
	{
		match state
		{
			TaskTrafficState::Generating => self.generating_tasks_histogram.entry(cycle as usize/self.box_size).and_modify(|e|{  e[task] = 1 }).or_insert({ let mut a= vec![0; self.tasks]; a[task]= 1; a } ),
			TaskTrafficState::UnspecifiedWait => self.waiting_tasks_histogram.entry(cycle as usize/self.box_size).and_modify(| e|{ e[task] = 1 }).or_insert({ let mut a= vec![0; self.tasks]; a[task]= 1; a } ),
			TaskTrafficState::FinishedGenerating => self.finished_generating_tasks_histogram.entry(cycle as usize/self.box_size).and_modify(| e|{ e[task] = 1 }).or_insert({ let mut a= vec![0; self.tasks]; a[task]= 1; a }),
			TaskTrafficState::Finished  => {
				self.finished_tasks_histogram.entry(cycle as usize / self.box_size).and_modify(|e| { e[task] = 1 }).or_insert({
					let mut a = vec![0; self.tasks];
					a[task] = 1;
					a
				});
				self.finished_generating_tasks_histogram.entry(cycle as usize / self.box_size).and_modify(|e| { e[task] = 1 }).or_insert({
					let mut a = vec![0; self.tasks];
					a[task] = 1;
					a
				})
			},
			TaskTrafficState::WaitingData => {
				self.waiting_data_histogram.entry(cycle as usize / self.box_size).and_modify(|e| { e[task] = 1 }).or_insert({
					let mut a = vec![0; self.tasks];
					a[task] = 1;
					a
				})
			},
			_ => panic!("Invalid task state to take metrics"), //| TaskTrafficState::WaitingData

		};
		if let Some(subtraffic) = subtraffic
		{
			if let Some(sub) = self.sub_traffic_statistics.as_mut()
			{
				sub[subtraffic].track_task_state(task, state, cycle, None);
			}else {
				panic!("Subtraffic statistics not initialized");
			}
		}
	}

	pub fn parse_statistics(&self) -> ConfigurationValue
	{
		let max = self.histogram_messages_delay.clone().into_keys().max().unwrap();
		let messages_latency_histogram = (0..max+1).map(|i|
			ConfigurationValue::Number(self.histogram_messages_delay.get(&i).unwrap_or(&0).clone() as f64)
		).collect();
		// let messages_network_latency_histogram = (0..max+1).map(|i|
		// 	ConfigurationValue::Number(self.histogram_messages_network_delay.get(&i).unwrap_or(&0).clone() as f64)
		// ).collect();

		//let max_tasks = cmp::max(cmp::max(self.generating_tasks_histogram.keys().max().unwrap_or(&0), self.waiting_tasks_histogram.keys().max().unwrap_or(&0)), self.finished_tasks_histogram.keys().max().unwrap_or(&0));
		let max_tasks = self.cycle_last_consumed_message as usize / self.box_size +1;
		let generated_tasks_histogram = (0..max_tasks+1).map(|i|
			ConfigurationValue::Number( self.generating_tasks_histogram.get(&i).unwrap_or(&vec![]).iter().map(|x|*x as f64).sum()
		)).collect();
		let waiting_tasks_histogram = (0..max_tasks+1).map(|i|
			ConfigurationValue::Number( self.waiting_tasks_histogram.get(&i).unwrap_or(&vec![]).iter().map(|x|*x as f64).sum()
		)).collect();
		let finished_generating_tasks_histogram = (0..max_tasks+1).map(|i|
			ConfigurationValue::Number( self.finished_generating_tasks_histogram.get(&i).unwrap_or(&vec![]).iter().map(|x|*x as f64).sum()
		)).collect();
		let finished_tasks_histogram = (0..max_tasks+1).map(|i|
			ConfigurationValue::Number( self.finished_tasks_histogram.get(&i).unwrap_or(&vec![]).iter().map(|x|*x as f64).sum()
		)).collect();

		let mut traffic_content = vec![
			(String::from("total_consumed_messages"),ConfigurationValue::Number(self.total_consumed_messages as f64)),
			(String::from("total_consumed_phits"),ConfigurationValue::Number(self.total_consumed_phits as f64)),
			(String::from("total_created_messages"),ConfigurationValue::Number(self.total_created_messages as f64)),
			(String::from("total_created_phits"),ConfigurationValue::Number(self.total_created_phits as f64)),
			(String::from("total_message_delay"),ConfigurationValue::Number((self.total_message_delay/cmp::max(self.total_consumed_messages as u64, 1u64))as f64)),
			(String::from("cycle_last_created_message"),ConfigurationValue::Number(self.cycle_last_created_message as f64)),
			(String::from("cycle_last_consumed_message"),ConfigurationValue::Number(self.cycle_last_consumed_message as f64)),
			(String::from("message_latency_histogram"),ConfigurationValue::Array(messages_latency_histogram)),
			// (String::from("message_network_latency_histogram"),ConfigurationValue::Array(messages_network_latency_histogram)),
			(String::from("generating_tasks_histogram"),ConfigurationValue::Array(generated_tasks_histogram)),
			(String::from("waiting_tasks_histogram"),ConfigurationValue::Array(waiting_tasks_histogram)),
			(String::from("finished_generating_tasks_histogram"),ConfigurationValue::Array(finished_generating_tasks_histogram)),
			(String::from("finished_tasks_histogram"),ConfigurationValue::Array(finished_tasks_histogram)),
		];
		if self.temporal_step > 0
		{
			let temporal_consumed_messages = self.temporal_statistics.iter().map(|m|ConfigurationValue::Number(m.consumed_messages as f64)).collect();
			let temporal_consumed_phits = self.temporal_statistics.iter().map(|m|ConfigurationValue::Number(m.consumed_phits as f64)).collect();
			let temporal_created_messages = self.temporal_statistics.iter().map(|m|ConfigurationValue::Number(m.created_messages as f64)).collect();
			let temporal_created_phits = self.temporal_statistics.iter().map(|m|ConfigurationValue::Number(m.created_phits as f64)).collect();
			let temporal_message_delay = self.temporal_statistics.iter().map(|m|
				if m.consumed_messages != 0 {
					ConfigurationValue::Number((m.total_message_delay/m.consumed_messages as u64)as f64)
				} else {
					ConfigurationValue::Number(0f64)
				}
			).collect();

			let temporal_content = vec![
				(String::from("consumed_messages"),ConfigurationValue::Array(temporal_consumed_messages)),
				(String::from("consumed_phits"),ConfigurationValue::Array(temporal_consumed_phits)),
				(String::from("created_messages"),ConfigurationValue::Array(temporal_created_messages)),
				(String::from("created_phits"),ConfigurationValue::Array(temporal_created_phits)),
				(String::from("message_delay"),ConfigurationValue::Array(temporal_message_delay)),
			];
			traffic_content.push((String::from("temporal"), ConfigurationValue::Object(String::from("temporal_statistics"),temporal_content)));
		}

		if let Some(sub) = &self.sub_traffic_statistics
		{
			let sub_content = sub.iter().map(|s|s.parse_statistics()).collect();
			traffic_content.push((String::from("sub_traffics"), ConfigurationValue::Array(sub_content)));
		}
		ConfigurationValue::Object(String::from("traffic_statistics"), traffic_content)
	}
}

///Statistics captured for each link.
#[derive(Debug,Quantifiable)]
pub struct LinkStatistics
{
	pub phit_arrivals: usize,
}

impl LinkStatistics
{
	fn new() -> LinkStatistics
	{
		LinkStatistics{
			phit_arrivals: 0,
		}
	}
	fn reset(&mut self)
	{
		self.phit_arrivals=0;
	}
}

///default() generates an empty measurement, invoked on each reset. `begin_cycle` must be set on resets.
#[derive(Debug,Default,Quantifiable)]
pub struct StatisticMeasurement
{
	///The number of the first cycle included in the statistics.
	pub begin_cycle: Time,
	///The number of phits that servers have sent to routers.
	pub created_phits: usize,
	///Number of phits that have reached their destination server (called consume).
	pub consumed_phits: usize,
	///Number of phit tails consumed.
	pub consumed_packets: usize,
	///Number of messages for which all their phits have beeen consumed.
	pub consumed_messages: usize,
	///Accumulated delay of al messages. From message creation (in the traffic module) to server consumption.
	pub total_message_delay: Time,
	///Accumulated network delay for all packets. From the leading phit being inserted into a router to the consumption of the tail phit.
	pub total_packet_network_delay: Time,
	///Accumulated count of hops made for all consumed packets.
	pub total_packet_hops: usize,
	///Count of consumed packets indexed by the number of hops it made.
	pub total_packet_per_hop_count: Vec<usize>,
	///For each virtual channel `vc`, `virtual_channel_usage[vc]` counts the total number of times
	///a phit has advanced by any link using that virtual channel.
	pub virtual_channel_usage: Vec<usize>,
}

//impl StatisticMeasurement
//{
//	//TODO: this do not use `self`, and does not work with temporal statistics.
//	pub fn jain_server_created_phits(&self, network:&Network) -> f64
//	{
//		//double rcvd_count_total=0.0;
//		//double rcvd_count2_total=0.0;
//		let mut count=0.0;
//		let mut count2=0.0;
//		for server in network.servers.iter()
//		{
//			//double x=(double)(network[i].rcvd_count_from);
//			let x=server.statistics.current_measurement.created_phits as f64;
//			count+=x;
//			count2+=x*x;
//		}
//		//double Jain_fairness=rcvd_count_total*rcvd_count_total/rcvd_count2_total/(double)nprocs;
//		//printf("OUT_Jain_fairness=%f%s",Jain_fairness,sep);
//		count*count/count2/network.servers.len() as f64
//	}
//	pub fn jain_server_consumed_phits(&self, network:&Network) -> f64
//	{
//		//double rcvd_count_total=0.0;
//		//double rcvd_count2_total=0.0;
//		let mut count=0.0;
//		let mut count2=0.0;
//		for server in network.servers.iter()
//		{
//			//double x=(double)(network[i].rcvd_count_from);
//			let x=server.statistics.current_measurement.consumed_phits as f64;
//			count+=x;
//			count2+=x*x;
//		}
//		//double Jain_fairness=rcvd_count_total*rcvd_count_total/rcvd_count2_total/(double)nprocs;
//		//printf("OUT_Jain_fairness=%f%s",Jain_fairness,sep);
//		count*count/count2/network.servers.len() as f64
//	}
//}

pub fn jain<I:Iterator<Item=f64>>(iter:I) -> f64
{
	let mut n = 0;
	let mut count=0.0;
	let mut count2=0.0;
	for x in iter
	{
		n+=1;
		//double x=(double)(network[i].rcvd_count_from);
		//let x=server.statistics.current_measurement.consumed_phits as f64;
		count+=x;
		count2+=x*x;
	}
	//double Jain_fairness=rcvd_count_total*rcvd_count_total/rcvd_count2_total/(double)nprocs;
	//printf("OUT_Jain_fairness=%f%s",Jain_fairness,sep);
	count*count/count2/n as f64
}


#[derive(Debug,Quantifiable)]
pub struct StatisticPacketMeasurement
{
	///The cycle in which the packet was consumed, including its tail phit.
	pub consumed_cycle: Time,
	///The number of router-to-router links the packet has traversed.
	pub hops: usize,
	///The number of cycles since the packet was created until it was consumed.
	pub delay: Time,
}

///All the global statistics captured.
#[derive(Debug,Quantifiable)]
pub struct Statistics
{
	//The stored path is used for some calls to `config::evaluate`.
	//path: PathBuf,
	///The measurement since the last reset.
	pub current_measurement: StatisticMeasurement,
	///Specific statistics of the links. Indexed by router and port.
	pub link_statistics: Vec<Vec<LinkStatistics>>,
	///If non-zero then creates statistics for intervals of the given number of cycles.
	pub temporal_step: Time,
	///The periodic measurements requested by non-zero statistics_temporal_step.
	pub temporal_statistics: Vec<StatisticMeasurement>,
	///For each percentile `perc` write server statistics for that percentile. This is, the lowest value such that `perc`% of the servers have lower value.
	///These values will appear in the `server_percentile{perc}` field of the result file.
	///For example, `server_percentile25.injected_load` will be a value with 25% of the servers generating less load and `server_percentile25.accepted_load` will be a value with 25% of the servers consuming less load. Note those values will probably correspond to different servers, despite being written into the same record.
	///The percentiles are integer numbers mostly to make obvious their representation in the name of the field.
	///The default value is empty.
	pub server_percentiles: Vec<u8>,
	///For each percentile `perc` write packet statistics for that percentile.
	pub packet_percentiles: Vec<u8>,
	///Data collected to show `packet_percentiles` if not empty.
	pub packet_statistics: Vec<StatisticPacketMeasurement>,
	///The columns to print in the periodic reports.
	pub columns: Vec<ReportColumn>,
	///A list of statistic definitions for consumed packets.
	///Each definition is a tuple `(keys,values)`, that are evaluated on each packet.
	///Packets are classified via `keys` into their bin. The number of packets in each bin is counted and the associated `values` are averaged.
	pub packet_defined_statistics_definitions: Vec< (Vec<Expr>,Vec<Expr>) >,
	///For each definition of packet statistics, we have a vector with an element for each actual value of `keys`.
	///Each of these elements have that value of `key`, together with the averages and the count.
	pub packet_defined_statistics_measurement: Vec< Vec< (Vec<ConfigurationValue>,Vec<f32>,usize) >>,
	///A list of statistic definitions for message statistics.
	/// Each definition is a tuple `(keys,values)`, that are evaluated on each message.
	/// Messages are classified via `keys` into their bin. The number of messages in each bin is counted and the associated `values` are averaged.
	pub message_defined_statistics_definitions: Vec< (Vec<Expr>,Vec<Expr>) >,
	///For each definition of message statistics, we have a vector with an element for each actual value of `keys`.
	/// Each of these elements have that value of `key`, together with the averages and the count.
	pub message_defined_statistics_measurement: Vec< Vec< (Vec<ConfigurationValue>,Vec<f32>,usize) >>,
	///A list of statistic definitions for server statistics, indexed by the temporal step.
	pub temporal_defined_statistics_definitions: Vec< (Vec<Expr>, Vec<Expr>) >,
	///For each definition of server statistics, we have a vector with an element for each actual value of `keys`.
	pub temporal_defined_statistics_measurement: Vec< Vec< Vec< (Vec<ConfigurationValue>, Vec<f32>, usize) >>>,
}

impl Statistics
{
	pub fn new(statistics_temporal_step:Time, server_percentiles: Vec<u8>, packet_percentiles: Vec<u8>, packet_defined_statistics_definitions:Vec<(Vec<Expr>, Vec<Expr>)>, message_defined_statistics_definitions:Vec<(Vec<Expr>, Vec<Expr>)>, temporal_defined_statistics_definitions:Vec<(Vec<Expr>, Vec<Expr>)>, topology: &dyn Topology) ->Statistics
	{
		let packet_defined_statistics_measurement = vec![vec![]; packet_defined_statistics_definitions.len() ];
		let message_defined_statistics_measurement = vec![vec![]; message_defined_statistics_definitions.len() ];
		let temporal_defined_statistics_measurement = vec![ vec![vec![]; temporal_defined_statistics_definitions.len() ] ];
		Statistics{
			//begin_cycle:0,
			//created_phits:0,
			//consumed_phits:0,
			//consumed_packets:0,
			//consumed_messages:0,
			//total_message_delay:0,
			//total_packet_hops:0,
			//total_packet_per_hop_count:Vec::new(),
			current_measurement: Default::default(),
			link_statistics: (0..topology.num_routers()).map(|i| (0..topology.ports(i)).map(|_|LinkStatistics::new()).collect() ).collect(),
			temporal_step: statistics_temporal_step,
			temporal_statistics: vec![],
			server_percentiles,
			packet_percentiles,
			packet_statistics: vec![],
			columns: vec![
				ReportColumnKind::BeginEndCycle.into(),
				ReportColumnKind::InjectedLoad.into(),
				ReportColumnKind::AcceptedLoad.into(),
				ReportColumnKind::AveragePacketHops.into(),
				ReportColumnKind::AverageLinkUtilization.into(),
				//ReportColumnKind::MaximumLinkUtilization.into(),
				ReportColumnKind::AverageMessageDelay.into(),
				ReportColumnKind::ServerGenerationJainIndex.into(),
				//ReportColumnKind::ServerConsumptionJainIndex.into(),
				],
			packet_defined_statistics_definitions,
			packet_defined_statistics_measurement,
			message_defined_statistics_definitions,
			message_defined_statistics_measurement,
			temporal_defined_statistics_definitions,
			temporal_defined_statistics_measurement,
		}
	}
	///Print in stdout a header showing the statistical columns to be periodically printed.
	pub fn print_header(&self)
	{
		//println!("cycle_begin-cycle_end injected_load accepted_load server_generation_jain_index server_consumption_jain_index");
		let report:String = self.columns.iter().map(|c|c.header()).collect();
		println!("{}",report);
	}
	///Print in stdout the current values of the statistical columns indicated to be periodically printed.
	pub fn print(&self, next_cycle:Time, network:&Network)
	{
		//let cycles=next_cycle-self.begin_cycle+1;
		//let injected_load=self.created_phits as f32/cycles as f32/network.servers.len() as f32;
		//let accepted_load=self.consumed_phits as f32/cycles as f32/network.servers.len() as f32;
		//let jsgp=self.jain_server_created_phits(network);
		//let jscp=self.jain_server_consumed_phits(network);
		//println!("{:>11}-{:<9} {:<13} {:<13} {:<17} {:<12}",self.begin_cycle,next_cycle-1,injected_load,accepted_load,jsgp,jscp);
		let report:String = self.columns.iter().map(|c|c.format(self,next_cycle,network)).collect();
		println!("{}",report);
	}
	///Forgets all captured statistics and began capturing again.
	pub fn reset(&mut self,next_cycle:Time, network:&mut Network)
	{
		//self.begin_cycle=next_cycle;
		//self.created_phits=0;
		//self.consumed_phits=0;
		//self.consumed_packets=0;
		//self.consumed_messages=0;
		//self.total_message_delay=0;
		//self.total_packet_hops=0;
		//self.total_packet_per_hop_count=Vec::new();
		self.current_measurement=Default::default();
		self.current_measurement.begin_cycle=next_cycle;
		for server in network.servers.iter_mut()
		{
			server.statistics.reset(next_cycle);
		}
		for router in network.routers.iter()
		{
			router.borrow_mut().reset_statistics(next_cycle);
		}
		for router_links in self.link_statistics.iter_mut()
		{
			for link in router_links.iter_mut()
			{
				link.reset();
			}
		}
	}
	/// Called each time a server consumes a phit.
	pub fn track_consumed_phit(&mut self, cycle: Time)
	{
		self.current_measurement.consumed_phits+=1;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.consumed_phits+=1;
		}
	}
	/// Called when a server consumes a tail phit.
	pub fn track_consumed_packet(&mut self, cycle: Time, packet:&Packet)
	{
		self.current_measurement.consumed_packets+=1;
		let network_delay = cycle-*packet.cycle_into_network.borrow();
		self.current_measurement.total_packet_network_delay += network_delay;
		let hops=packet.routing_info.borrow().hops;
		self.current_measurement.total_packet_hops+=hops;
		if self.current_measurement.total_packet_per_hop_count.len() <= hops
		{
			self.current_measurement.total_packet_per_hop_count.resize( hops+1, 0 );
		}
		self.current_measurement.total_packet_per_hop_count[hops]+=1;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.consumed_packets+=1;
			m.total_packet_network_delay+=network_delay;
			m.total_packet_hops+=hops;
		}
		if !self.packet_percentiles.is_empty()
		{
			self.packet_statistics.push(StatisticPacketMeasurement{consumed_cycle:cycle,hops,delay:network_delay});
		}
		if !self.packet_defined_statistics_definitions.is_empty()
		{
			let be = packet.extra.borrow();
			let extra = be.as_ref().unwrap();
			let link_classes = extra.link_classes.iter().map(|x|ConfigurationValue::Number(*x as f64)).collect();
			let switches = extra.id_switches.iter().map(|x|ConfigurationValue::Number(*x as f64)).collect();
			let entry_virtual_channels = extra.entry_virtual_channels.iter().map(|x|match x{
				Some(v) => ConfigurationValue::Number(*v as f64),
				None => ConfigurationValue::None,
			}).collect();
			let cycle_per_hop = extra.cycle_per_hop.iter().map(|x|ConfigurationValue::Number(*x as f64)).collect();
			let context_content = vec![
				(String::from("hops"), ConfigurationValue::Number(hops as f64)),
				(String::from("delay"), ConfigurationValue::Number(network_delay as f64)),
				(String::from("cycle_into_network"), ConfigurationValue::Number(*packet.cycle_into_network.borrow() as f64)),
				(String::from("size"), ConfigurationValue::Number(packet.size as f64)),
				(String::from("link_classes"), ConfigurationValue::Array(link_classes)),
				(String::from("switches"), ConfigurationValue::Array(switches)),
				(String::from("entry_virtual_channels"), ConfigurationValue::Array(entry_virtual_channels)),
				(String::from("cycle_per_hop"), ConfigurationValue::Array(cycle_per_hop)),
			];
			let context = ConfigurationValue::Object( String::from("packet"), context_content );
			let path = Path::new(".");
			for (index,definition) in self.packet_defined_statistics_definitions.iter().enumerate()
			{
				let key : Vec<ConfigurationValue> = definition.0.iter().map(|key_expr|config::evaluate( key_expr, &context, path).unwrap_or_else(|error|panic!("error building user defined statistics: {}",error))).collect();
				let value : Vec<f32> = definition.1.iter().map(|key_expr|
					match config::evaluate( key_expr, &context, path).unwrap_or_else(|error|panic!("error building user defined statistics: {}",error)){
						ConfigurationValue::Number(x) => x as f32,
						_ => 0f32,
					}).collect();
				//find the measurement
				let measurement = self.packet_defined_statistics_measurement[index].iter_mut().find(|m|m.0==key);
				match measurement
				{
					Some(m) =>
					{
						for (iv,v) in m.1.iter_mut().enumerate()
						{
							*v += value[iv];
						}
						m.2+=1;
					}
					None => {
						self.packet_defined_statistics_measurement[index].push( (key,value,1) )
					},
				};
			}
		}
	}
	/// Called when a server consumes the last phit from a message.
	pub fn track_consumed_message(&mut self, cycle: Time)
	{
		self.current_measurement.consumed_messages+=1;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.consumed_messages+=1;
		}
	}
	/// Called each time a phit is created.
	pub fn track_created_phit(&mut self, cycle: Time)
	{
		self.current_measurement.created_phits+=1;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.created_phits+=1;
		}
	}
	/// Called when a server consumes the last phit from a message.
	/// XXX: Perhaps this should be part of `track_consumed_message`.
	pub fn track_message_delay(&mut self, delay:Time, cycle: Time)
	{
		self.current_measurement.total_message_delay+= delay;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			m.total_message_delay+=delay;
		}

		if !self.message_defined_statistics_definitions.is_empty()
		{
			let context_content = vec![
				(String::from("delay"), ConfigurationValue::Number(delay as f64)),
			];
			let context = ConfigurationValue::Object( String::from("message"), context_content );
			let path = Path::new(".");
			for (index,definition) in self.message_defined_statistics_definitions.iter().enumerate()
			{
				let key : Vec<ConfigurationValue> = definition.0.iter().map(|key_expr|config::evaluate( key_expr, &context, path).unwrap_or_else(|error|panic!("error building user defined statistics: {}",error))).collect();
				let value : Vec<f32> = definition.1.iter().map(|key_expr|
					match config::evaluate( key_expr, &context, path).unwrap_or_else(|error|panic!("error building user defined statistics: {}",error)){
						ConfigurationValue::Number(x) => x as f32,
						_ => 0f32,
					}).collect();
				//find the measurement
				let measurement = self.message_defined_statistics_measurement[index].iter_mut().find(|m|m.0==key);
				match measurement
				{
					Some(m) =>
					{
						for (iv,v) in m.1.iter_mut().enumerate()
						{
							*v += value[iv];
						}
						m.2+=1;
					}
					None => {
						self.message_defined_statistics_measurement[index].push( (key,value,1) )
					},
				};
			}
		}
	}
	/// Called with a hop from router to router
	pub fn track_phit_hop(&mut self, phit:&Phit, cycle: Time)
	{
		let vc:usize = phit.virtual_channel.borrow().unwrap();
		if self.current_measurement.virtual_channel_usage.len() <= vc
		{
			self.current_measurement.virtual_channel_usage.resize(vc+1, 0);
		}
		self.current_measurement.virtual_channel_usage[vc]+=1;
		if let Some(m) = self.current_temporal_measurement(cycle)
		{
			if m.virtual_channel_usage.len() <= vc
			{
				m.virtual_channel_usage.resize(vc+1, 0);
			}
			m.virtual_channel_usage[vc]+=1;
		}
	}
	//fn track_packet_hops(&mut self, hops:usize, cycle: Time)
	//{
	//	self.current_measurement.total_packet_hops+=hops;
	//	if self.current_measurement.total_packet_per_hop_count.len() <= hops
	//	{
	//		self.current_measurement.total_packet_per_hop_count.resize( hops+1, 0 );
	//	}
	//	self.current_measurement.total_packet_per_hop_count[hops]+=1;
	//	if self.temporal_step>0
	//	{
	//		let index = cycle / self.temporal_step;
	//		if self.temporal_statistics.len()<=index
	//		{
	//			self.temporal_statistics.resize_with(index+1,Default::default);
	//			self.temporal_statistics[index].begin_cycle = index*self.temporal_step;
	//		}
	//		self.temporal_statistics[index].total_packet_hops+=hops;
	//		//Is total_packet_per_hop_count too much here?
	//	}
	//}
	pub fn current_temporal_measurement(&mut self, cycle: Time) -> Option<&mut StatisticMeasurement>
	{
		if self.temporal_step>0
		{
			let index : usize = (cycle / self.temporal_step).try_into().unwrap();
			if self.temporal_statistics.len()<=index
			{
				self.temporal_statistics.resize_with(index+1,Default::default);
				self.temporal_statistics[index].begin_cycle = index as Time * self.temporal_step;
			}
			Some(&mut self.temporal_statistics[index])
		} else { None }
	}
}

///The available statistical columns. Each column has a string for the header and a way to compute what to print each period.
#[derive(Debug,Quantifiable)]
#[allow(dead_code)]
enum ReportColumnKind
{
	BeginEndCycle,
	InjectedLoad,
	AcceptedLoad,
	ServerGenerationJainIndex,
	ServerConsumptionJainIndex,
	AverageMessageDelay,
	AveragePacketNetworkDelay,
	AveragePacketHops,
	AverageLinkUtilization,
	MaximumLinkUtilization,
}

impl ReportColumnKind
{
	fn name(&self) -> &str
	{
		match self
		{
			ReportColumnKind::BeginEndCycle => "cycle_begin-cycle_end",
			ReportColumnKind::InjectedLoad => "injected_load",
			ReportColumnKind::AcceptedLoad => "accepted_load",
			ReportColumnKind::ServerGenerationJainIndex => "server_generation_jain_index",
			ReportColumnKind::ServerConsumptionJainIndex => "server_consumption_jain_index",
			ReportColumnKind::AverageMessageDelay => "average_message_delay",
			ReportColumnKind::AveragePacketNetworkDelay => "average_packet_network_delay",
			ReportColumnKind::AveragePacketHops => "average_packet_hops",
			ReportColumnKind::AverageLinkUtilization => "average_link_utilization",
			ReportColumnKind::MaximumLinkUtilization => "maximum_link_utilization",
		}
	}
}

///A statistical column with extra formatting information.
#[derive(Debug,Quantifiable)]
pub struct ReportColumn
{
	kind: ReportColumnKind,
	width: usize,
}

impl ReportColumn
{
	fn header(&self) -> String
	{
		//let base = match self.kind
		//{
		//	ReportColumnKind::BeginEndCycle => "cycle_begin-cycle_end",
		//	ReportColumnKind::InjectedLoad => "injected_load",
		//	ReportColumnKind::AcceptedLoad => "accepted_load",
		//	ReportColumnKind::ServerGenerationJainIndex => "server_generation_jain_index",
		//	ReportColumnKind::ServerConsumptionJainIndex => "server_consumption_jain_index",
		//};
		let base = self.kind.name();
		format!("{name:width$}",name=base,width=self.width)
	}
	fn format(&self, statistics: &Statistics, next_cycle: Time, network:&Network) -> String
	{
		let cycles=next_cycle-statistics.current_measurement.begin_cycle+1;
		let value = match self.kind
		{
			ReportColumnKind::BeginEndCycle => format!("{:>11}-{}",statistics.current_measurement.begin_cycle,next_cycle-1),
			ReportColumnKind::InjectedLoad => format!{"{}",statistics.current_measurement.created_phits as f32/cycles as f32/network.servers.len() as f32},
			ReportColumnKind::AcceptedLoad =>  format!{"{}",statistics.current_measurement.consumed_phits as f32/cycles as f32/network.servers.len() as f32},
			ReportColumnKind::ServerGenerationJainIndex => format!{"{}",network.jain_server_created_phits()},
			ReportColumnKind::ServerConsumptionJainIndex => format!{"{}",network.jain_server_consumed_phits()},
			ReportColumnKind::AverageMessageDelay => format!("{}",statistics.current_measurement.total_message_delay as f64/statistics.current_measurement.consumed_messages as f64),
			ReportColumnKind::AveragePacketNetworkDelay => format!("{}",statistics.current_measurement.total_packet_network_delay as f64/statistics.current_measurement.consumed_packets as f64),
			ReportColumnKind::AveragePacketHops => format!("{}",statistics.current_measurement.total_packet_hops as f64 / statistics.current_measurement.consumed_packets as f64),
			ReportColumnKind::AverageLinkUtilization =>
			{
				let total_arrivals:usize = (0..network.topology.num_routers()).map(|i|(0..network.topology.degree(i)).map(|j|statistics.link_statistics[i][j].phit_arrivals).sum::<usize>()).sum();
				let total_links: usize = (0..network.topology.num_routers()).map(|i|network.topology.degree(i)).sum();
				format!("{}",total_arrivals as f64 / cycles as f64 / total_links as f64)
			},
			ReportColumnKind::MaximumLinkUtilization =>
			{
				let maximum_arrivals:usize = statistics.link_statistics.iter().map(|rls|rls.iter().map(|ls|ls.phit_arrivals).max().unwrap()).max().unwrap();
				format!("{}",maximum_arrivals as f64 / cycles as f64)
			},
		};
		format!("{value:width$}",value=value,width=self.width)
	}
}

///From putting default values for each kind.
impl From<ReportColumnKind> for ReportColumn
{
	fn from(kind:ReportColumnKind) -> ReportColumn
	{
		let width = 1+kind.name().len();
		ReportColumn{
			kind,
			width,
		}
	}
}


