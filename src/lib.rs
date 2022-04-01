/*!
caminos-lib
=====

This crate provides the CAMINOS simulator as a library. This is the Cantabrian Adaptable and Modular Interconnection Open Simulator.

# Usage

This crate is `caminos-lib`. To use it add `caminos-lib` to your dependencies in your project's `Cargo.toml`.

```toml
[dependencies]
caminos-lib = "0.4"
```

Alternatively, consider whether the binary crate `caminos` fits your intended use.

# Breaking changes

<details>

## [0.4.0] to

* Added the function `server_state` to the `Traffic` trait.
* Functions on the output module now use ExperimentFiles instead of Path.
* Added a server argument to `Traffic::try_consume`.
* Added phit to `RequestInfo`.
* Upgrade from rand-0.4 to rand-0.8.

## [0.3.0] to [0.4.0]

* Added `path` argument to `config::{evaluate,reevaluate}`.
* File `create_output` and similar now receive in its `results` argument also the experiment indices.
* routings now return `RoutingNextCandidates`. In addition to the vector of candidates it contains an `idempotent` field to allow some checks and optimizations.
* Added requirement `VirtualChannelPolicy: Debug`.
* The `file_main` function now receives a `free_args` parameter. Free arguments of the form `path=value` are used to override the configuration.

## [0.2.0] to [0.3.0]

* Added parameter `cycle` to `Traffic::should_generate`.

## [0.1.0] to [0.2.0]

* Added methods to `Routing` and `Router` traits to gather statistics.
* Added method `Routing::performed_request` to allow routings to make decisions when the router makes a request to a candidate.
* Added `ConfigurationValue::NamedExperiments(String,Vec<ConfigurationValue>)`.
* Removed surrounding quotes from the config `LitStr` and `Literal`.
* Now `neighbour_router_iter` must always be used instead of `0..degree()` to check ports to other routers. Note that `degree`  does not give valid ranges when having non-connected ports, as in the case of some irregular topologies as the mesh.
* `Plugs` now include a `stages` attribute.
* Removed from the `Topology` interfaz the never used methods `num_arcs`, `average_distance`, `distance_distribution`.
</details>

# Public Interface

`caminos-lib` provides the functions `directory_main` and `file_main`, intended to use the file version when the final binary calls with a configuration file argument and the directory version when it is called with a directory argument.

The `directory_main` function receives a `&Path` assumed to contain a `main.cfg`, `main.od`, optionally `remote`, plus any generated files and subdirectories.
* `main.cfg` contains the definition of the experiment to perform, expected to unfold into multiple simulations.
* `main.od` contains the definition of what outputs are desired. For example `csv` files or (`pdf`,`latex`)-plots.
* `remote` allows to define a remote from which to pull result files.
* `journal`tracks the actions performed on the experiment. It is specially useful to track what execution are currently launched in what slurm jobs.
* `runs/job<action_index>/launch<experiment_index>` are the scripts launched to slurm. `action_index` is number of the current action. `experiment_index` is expected to be the experiment index of one of the experiments included in the slurm job.
* `runs/job<action_index>/launch<experiment_index>-<slurm_index>.{out,err}` are the outputs from scripts launched to slurm. The `slurm_index` is the job id given by slurm.
* `runs/run<experiment_index>/local.cfg` is the configuration exclusive to the simulation number `experiment_index`.
* `runs/run<experiment_index>/local.result` will contain the result values of the simulation number `experiment_index` after a successful simulation.

The `directory_main` receives also an `Action`. In the crate `caminos` this is done via its `--action=<method>` falg.
* `local_and_output` runs all the remaining simulations locally and generates the outputs.
* `local` runs all the simulations locally, without processing the results afterwards.
* `output` processes the currently available results and generates the outputs.
* `slurm` launches the remaining simulations onto the slurm system.
* `check` just shows how many results we got and how many are currently in slurm.
* `pull` brings result files from the defined remote host.
* `remote_check` performs a `check` action in the remote host.
* `push` compares the local main.cfg with the host remote.cfg. It reports discrepancies and create the remote path if missing.
* `slurm_cancel` executes a `scancel` with the job ids found in the journal file.
* `shell` creates the experiment folder with default configuration files. Alternatively, when receiving `--source=another_experiment` it copies the configuration of the other experiment into this one.
* `pack` forces the creation of a binary.results file and erases the verbose raw results files. In some extreme cases it can reduce a decent amount of space and sped up computations.


# Configuration Syntax

The configuration files are parsed using the `gramatica` crate. These files are parsed as a `ConfigurationValue` defined as following.

```
pub enum ConfigurationValue
{
	Literal(String),
	Number(f64),
	Object(String,Vec<(String,ConfigurationValue)>),
	Array(Vec<ConfigurationValue>),
	Experiments(Vec<ConfigurationValue>),
	NamedExperiments(String,Vec<ConfigurationValue>),
	True,
	False,
	Where(Rc<ConfigurationValue>,Expr),
	Expression(Expr),
}
```

* An `Object` os typed `Name { key1 : value1, key2 : value2, [...] }`.
* An `Array` is typed `[value1, value2, value3, [...]]`.
* An `Experiments` is typed `![value1, value2, value3, [...]]`. These are used to indicate several simulations in a experiment. This is, the set of simulations to be performed is the product of all lists of this kind.
* A `NamedExperiments`is typed `username![value1, value2, value3, [...]]`. Its size must match other `NamedExperiment`s with the same name. Thus if there is `{firstkey: alpha![value1, value2, value3],secondkey: alpha![other1,other2,other3]}`, then the simulations will include `{firstkey:value1, secondkey:other1}` and `{firstkey:value3,secondkey:other3}` but it will NOT include `{firstkey:value1,secondkey:other3}`.
* A `Number` can be written like 2 or 3.1. Stored as a `f64`.
* A `Literal` is a double-quoted string.
* `True` is written `true`a and `False` is written `false`.
* `Expression` is typed `=expr`, useful in output descriptions.
* The `Where` clause is not yet implemented.

## Experiment example

An example of `main.cfg` file is

```
Configuration
{
	random_seed: ![42,43,44],//Simulate each seed
	warmup: 20000,//Cycles to warm the network
	measured: 10000,//Cycles measured for the results
	topology: RandomRegularGraph//The topology is given as a named record
	{
		servers_per_router: 5,//Number of host connected to each router
		routers: 500,//Total number of routers in the network
		degree: 10,//Number of router ports reserved to go to other routers
		legend_name: "random 500-regular graph",//Name used on generated outputs
	},
	traffic: HomogeneousTraffic//Select a traffic. e.g., traffic repeating a pattern continously.
	{
		pattern: ![//We can make a simulation for each of several patterns.
			Uniform { legend_name:"uniform" },
			RandomPermutation { legend_name:"random server permutation" },
		],
		servers: 2500,//Servers involved in the traffic. Typically equal to the total of servers.
		//The load offered from the servers. A common case where to include many simulation values.
		load: ![0.05, 0.1, 0.15, 0.2, 0.25, 0.3, 0.35, 0.4, 0.45, 0.5, 0.55, 0.6, 0.65, 0.7, 0.75, 0.8, 0.85, 0.9, 0.95, 1.0],
		message_size: 16,//The size in phits of the messages created by the servers.
	},
	maximum_packet_size: 16,//Messages of greater length will be broken into several packets.
	router: Basic//The router is another object with a large description
	{
		//The number of virtual channels. The basic router sets a buffer for each virtual channel in each port, both at input and output.
		virtual_channels: 8,
		//Policies that filter the candidate routes given by the routing algorithm. They may be used to break deadlock or to give preference to some choices.
		//EnforceFlowControl must be included to actually use flow control restrictions.
		virtual_channel_policies: [ EnforceFlowControl, WideHops{width:1}, LowestSinghWeight{extra_congestion:0, extra_distance:0, aggregate_buffers:true, use_internal_space:true}, Random ],
		delay: 0,//not actually implemted in the basic router. In the future it may be removed or actually implemented.
		buffer_size: 64,//phits available in each input buffer
		bubble: false,//to enable bubble mechanism in Cartesian topologies.
		flit_size: 16,//set to maximum_packet_size to have Virtual Cut-Through.
		intransit_priority: false,//whether to give preference to transit over injection.
		allow_request_busy_port: true,//whether to allow input buffer to make requests to ports that are transmitting
		output_buffer_size:32,//Available phits in each output_buffer.
		output_priorize_lowest_label: true,//whether arbiters give priority to requests with lowest label.
	},
	routing: ![//Algorithm to provide candidate exit ports.
		Shortest { legend_name: "shortest" },
		Valiant {
			//The meta-routing by Valiant in which we sent shortest to a random middle router
			//And then shortest from the middle to the destination.
			first: Shortest,//We can change the sub-routing in either the first or second segment.
			second: Shortest,//If we do not have arguments we only put the object name. No need for braces.
			legend_name: "generic Valiant",
		},
	],
	link_classes: [
		//We can set the delays of different class of links. The number of classes depends on the topology.
		LinkClass {
			//In random regular graphs all router--router links have the same class.
			delay:1,
		},
		//The last class always correspond to the links between server and router
		LinkClass { delay: 1},
		//In a dragonfly topology we would have 0=server, 1=routers from same group, 2=routers from different groups.
	],
	launch_configurations: [
		//We may put here options to send to the SLURM system.
		Slurm
		{
			job_pack_size: 2,//number of simulations to go in each slurm job.
			time: "1-11:59:59",//maximum time allocated to each slurm job.
		},
	],
}
```

## Example output description

An example of output decription `main.od` is
```
[
	CSV//To generate a csv with a selection of fields
	{
		fields: [=configuration.traffic.pattern.legend_name, =configuration.traffic.load, =result.accepted_load, =result.average_message_delay, =configuration.routing.legend_name, =result.server_consumption_jain_index, =result.server_generation_jain_index, =result.average_packet_hops, =result.average_link_utilization, =result.maximum_link_utilization],
		filename: "results.csv",
	},
	Plots//To plot curves of data.
	{
		selector: =configuration.traffic.pattern.legend_name,//Make a plot for each value of the selector
		kind: [
			//We may create groups of figures.
			//In this example. For each value of pattern we draw three graphics.
			Plotkind{
				//The first one is accepted load for each offered load.
				//Simulations with same parameter, here offered load, are averaged together.
				parameter: =configuration.traffic.load,
				abscissas: =configuration.traffic.load,
				label_abscissas: "offered load",
				ordinates: =result.accepted_load,
				label_ordinates: "accepted load",
				min_ordinate: 0.0,
				max_ordinate: 1.0,
			},
			//In this example we draw message delay against accepted load, but we
			//continue to average by offered load. The offered load is also used for
			//the order in which points are joined by lines.
			Plotkind{
				parameter: =configuration.traffic.load,
				abscissas: =result.accepted_load,
				label_abscissas: "accepted load",
				ordinates: =result.average_message_delay,
				label_ordinates: "average message delay",
				min_ordinate: 0.0,
				max_ordinate: 200.0,
			},
		],
		legend: =configuration.routing.legend_name,
		prefix: "loaddelay",
		backend: Tikz
		{
			//We use tikz to create the figures.
			//We generate a tex file easy to embed in latex document.
			//We also generate apdf file, using the latex in the system.
			tex_filename: "load_and_delay.tex",
			pdf_filename: "load_and_delay.pdf",
		},
	},
	Plots
	{
		selector: =configuration.traffic.pattern.legend_name,//Make a plot for each value of the selector
		//We can create histograms.
		kind: [Plotkind{
			label_abscissas: "path length",
			label_ordinates: "amount fo packets",
			histogram: =result.total_packet_per_hop_count,
			min_ordinate: 0.0,
			//max_ordinate: 1.0,
		}],
		legend: =configuration.routing.legend_name,
		prefix: "hophistogram",
		backend: Tikz
		{
			tex_filename: "hop_histogram.tex",
			pdf_filename: "hop_histogram.pdf",
		},
	},
]
```

Fot the `tikz` backend to work it is necessary to have a working `LaTeX` installation that included the `pgfplots` package. It is part of the `texlive-pictures` package in some linux distributions.

# Plugging

Both entries `directory_main` and `file_main` receive a `&Plugs` argument that may be used to provide the simulator with new implementations. This way, one can make a copy of the `main` in the `caminos` crate and declare plugs for their implemented `Router`, `Topology`, `Routing`, `Traffic`, `Pattern`, and `VirtualChannelPolicy`.

*/

pub use quantifiable_derive::Quantifiable;//the derive macro

pub mod config_parser;
pub mod topology;
pub mod traffic;
pub mod pattern;
pub mod router;
pub mod routing;
pub mod event;
pub mod matrix;
pub mod output;
pub mod quantify;
pub mod policies;
pub mod experiments;
pub mod config;
pub mod error;

use std::rc::Rc;
use std::boxed::Box;
use std::cell::{RefCell};
use std::env;
use std::fs::{self,File};
use std::io::prelude::*;
use std::io::{stdout};
use std::collections::{VecDeque,BTreeMap};
use std::ops::DerefMut;
use std::path::{Path};
use std::mem::{size_of};
use std::fmt::Debug;
use std::convert::TryFrom;
use std::cmp::Ordering;
//use std::default::default;
//use std::borrow::Cow;
use rand::{rngs::StdRng,SeedableRng};

use config_parser::{ConfigurationValue,Expr};
use topology::{Topology,new_topology,TopologyBuilderArgument,Location,
	multistage::{Stage,StageBuilderArgument}};
use traffic::{Traffic,new_traffic,TrafficBuilderArgument,TrafficError};
use router::{Router,new_router,RouterBuilderArgument,TransmissionFromServer,TransmissionMechanism,StatusAtEmissor};
use routing::{RoutingInfo,Routing,new_routing,RoutingBuilderArgument};
use event::{EventQueue,Event};
use quantify::Quantifiable;
use experiments::{Experiment,Action,ExperimentOptions};
use policies::{VirtualChannelPolicy,VCPolicyBuilderArgument};
use pattern::{Pattern,PatternBuilderArgument};
use config::flatten_configuration_value;

#[derive(Clone,Quantifiable)]
struct ServerStatistics
{
	created_phits: usize,
	consumed_phits: usize,
	consumed_messages: usize,
	total_message_delay: usize,
	///The last cycle in which this server created a phit and sent it to a router. Or 0
	cycle_last_created_phit: usize,
	///The last cycle in that the last phit of a message has been consumed by this server. Or 0.
	cycle_last_consumed_message: usize,
	///Number of times the traffic returned true from `should_generate`, but it could not be stored.
	missed_generations: usize,
}

impl ServerStatistics
{
	fn new()->ServerStatistics
	{
		ServerStatistics{
			created_phits:0,
			consumed_phits:0,
			consumed_messages:0,
			total_message_delay:0,
			cycle_last_created_phit: 0,
			cycle_last_consumed_message: 0,
			missed_generations: 0,
		}
	}
	fn reset(&mut self)
	{
		self.created_phits=0;
		self.consumed_phits=0;
		self.consumed_messages=0;
		self.total_message_delay=0;
		self.missed_generations=0;
	}
}

///The objects that create and consume traffic to/from the network.
#[derive(Clone,Quantifiable)]
pub struct Server
{
	///The index of the server in the network.
	index: usize,
	///To which router the server is connected + link class index. Although we could just compute with the topology each time...
	port: (Location,usize),
	///Known available capacity in the connected router.
	router_status: router::StatusAtServer,
	///Created messages but not sent.
	stored_messages: VecDeque<Rc<Message>>,
	///The packets of the message that have not yet been sent.
	stored_packets: VecDeque<Rc<Packet>>,
	///The phits of a packet being sent.
	stored_phits: VecDeque<Rc<Phit>>,
	///For each message we store the number of consumed phits, until the whole message is consumed.
	consumed_phits: BTreeMap<*const Message,usize>,
	///Statistics local to the server.
	statistics: ServerStatistics,
}

impl Server
{
	///Consumes a phit
	fn consume(&mut self, phit:Rc<Phit>, traffic:&mut dyn Traffic, statistics:&mut Statistics, cycle:usize, topology:&Box<dyn Topology>, rng: &RefCell<StdRng>)
	{
		self.statistics.consumed_phits+=1;
		//statistics.consumed_phits+=1;
		statistics.track_consumed_phit(cycle);
		let message=phit.packet.message.clone();
		let message_ptr=message.as_ref() as *const Message;
		//println!("phit consumed at server {}: stats {:?}",self.index,statistics);
		let cp=match self.consumed_phits.get(&message_ptr)
		{
			None => 1,
			Some(x) => x+1,
		};
		if cp==message.size
		{
			//The whole message has been consumed
			self.statistics.consumed_messages+=1;
			//statistics.consumed_messages+=1;
			statistics.track_consumed_message(cycle);
			self.statistics.total_message_delay+=cycle-message.creation_cycle;
			self.statistics.cycle_last_consumed_message = cycle;
			//statistics.total_message_delay+=cycle-message.creation_cycle;
			statistics.track_message_delay(cycle-message.creation_cycle,cycle);
			self.consumed_phits.remove(&message_ptr);
			if !traffic.try_consume(self.index,message,cycle,topology,rng)
			{
				panic!("The traffic could not consume its own message.");
			}
			if !phit.is_end()
			{
				panic!("message was consumed by a non-ending phit.");
			}
		}
		else
		{
			self.consumed_phits.insert(message_ptr,cp);
			// Usually len==1
			//let n=self.consumed_phits.len();
			//if n>1
			//{
			//	println!("server.consumed_phits.len()={}",n);
			//}
		}
		if phit.is_end()
		{
			//statistics.consumed_packets+=1;
			statistics.track_consumed_packet(cycle,&phit.packet);
			//let hops=phit.packet.routing_info.borrow().hops;
			//statistics.total_packet_hops+=hops;
			//statistics.track_packet_hops(hops,cycle);
			//if statistics.total_packet_per_hop_count.len() <= hops
			//{
			//	statistics.total_packet_per_hop_count.resize( hops+1, 0 );
			//}
			//statistics.total_packet_per_hop_count[hops]+=1;
			if cp < phit.packet.size
			{
				println!("phit tail has been consuming without haing consumed a whole packet.");
			}
		}
	}
}

//impl Quantifiable for Server
//{
//	fn total_memory(&self) -> usize
//	{
//		return size_of::<Server>() + self.stored_messages.total_memory() + self.stored_packets.total_memory() + self.stored_phits.total_memory() + self.consumed_phits.total_memory();
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


///An instantiated network, with all its routers and servers.
pub struct Network
{
	///The topology defining the conectivity.
	pub topology: Box<dyn Topology>,
	//XXX The only reason to use Rc instead of Box is to make them insertable on the event queue. Perhaps the Eventful should be Box<MyRouter> instead of directly MyRouter? Or maybe storing some other kind of reference to the RefCell or the Box?
	///TThe collection of all the routers in the network.
	pub routers: Vec<Rc<RefCell<dyn Router>>>,
	//routers: Vec<Box<RefCell<dyn Router>>>,
	///TThe collection of all the servers in the network.
	pub servers: Vec<Server>,
}

impl Quantifiable for Network
{
	fn total_memory(&self) -> usize
	{
		let mut total=size_of::<Box<dyn Topology>>() + self.topology.total_memory() + self.routers.total_memory() + self.servers.total_memory();
		//let mut phit_count=0;
		for router in self.routers.iter()
		{
			total+=router.as_ref().total_memory();
			let rb=router.borrow();
			for phit in rb.iter_phits()
			{
				total+=phit.as_ref().total_memory();
				//phit_count+=1;
				if phit.is_end()
				{
					let packet=phit.packet.as_ref();
					total+=packet.total_memory();
				}
			}
		}
		for server in self.servers.iter()
		{
			for phit in server.stored_phits.iter()
			{
				total+=phit.as_ref().total_memory();
			}
			for packet in server.stored_packets.iter()
			{
				total+=packet.as_ref().total_memory();
			}
			for message in server.stored_messages.iter()
			{
				total+=message.as_ref().total_memory();
			}
			for (_message_ptr,_) in server.consumed_phits.iter()
			{
				total+=size_of::<Message>();
			}
		}
		//println!("phit_count={}",phit_count);
		total
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

///Minimal unit to be processed by the network.
///Not to be confused with flits.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Phit
{
	///The packet to what this phit belongs
	pub packet: Rc<Packet>,
	///position inside the packet
	pub index: usize,
	///The virtual channel in which this phit should be inserted
	pub virtual_channel: RefCell<Option<usize>>,
}


#[derive(Quantifiable)]
#[derive(Debug,Default)]
pub struct PacketExtraInfo
{
	link_classes: Vec<usize>,
	entry_virtual_channels: Vec<Option<usize>>,
	cycle_per_hop: Vec<usize>,
}

///A portion of a message. They are divided into phits.
///All phits must go through the same queues without phits of other packets in between.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Packet
{
	///Number of phits
	pub size: usize,
	///Information for the routing
	pub routing_info: RefCell<RoutingInfo>,
	///The message to what this packet belongs.
	pub message: Rc<Message>,
	///position inside the message
	pub index: usize,
	///The cycle when the packet has touched the first router. This is, the packet leading phit has been inserted into a router.
	///We set it to 0 if the packet has not entered the network yet.
	pub cycle_into_network: RefCell<usize>,
	///Extra info tracked for some special statistics.
	pub extra: RefCell<Option<PacketExtraInfo>>,
}

///An application message, broken into packets
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Message
{
	///Server that created the message.
	pub origin: usize,
	///Server that is the destination of the message.
	pub destination: usize,
	///Number of phits.
	pub size: usize,
	///Cycle when the message was created.
	pub creation_cycle: usize,
}

impl Phit
{
	///Whether the phit is leading a packet. Routers check this to make requests, stablish flows, etc.
	pub fn is_begin(&self) -> bool
	{
		self.index==0
	}
	///Whether this phit is the last one of a packet. Routers use this to finalize some operations.
	pub fn is_end(&self) -> bool
	{
		self.index==self.packet.size-1
	}
}

///Description of common properties of sets of links.
///For example, the links to servers could have a different delay.
///The topologies can set additional classes. For example, a mesh/torus can diffentiate horizontal/vertical links.
///And a dragonfly topology can differentiate local from global links.
pub struct LinkClass
{
	///Cycles the phit needs to move from one endpoint to the other endpoint.
	pub delay: usize,
	//(x,y) means x phits each y cycles ??
	//transference_speed: (usize,usize)
}

impl LinkClass
{
	fn new(cv:&ConfigurationValue) -> LinkClass
	{
		let mut delay=None;
		match_object_panic!(cv,"LinkClass",value,
			"delay" => delay=Some(value.as_f64().expect("bad value for delay") as usize),
			"transference_speed" => (),//FIXME
		);
		let delay=delay.expect("There were no delay");
		LinkClass{
			delay,
		}
	}
}

///Statistics captured for each link.
#[derive(Debug)]
struct LinkStatistics
{
	phit_arrivals: usize,
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
#[derive(Debug,Default)]
pub struct StatisticMeasurement
{
	///The number of the first cycle included in the statistics.
	begin_cycle: usize,
	///The number of phits that servers have sent to routers.
	created_phits: usize,
	///Number of phits that have reached their destination server (called consume).
	consumed_phits: usize,
	///Number of phit tails consumed.
	consumed_packets: usize,
	///Number of messages for which all their phits have beeen consumed.
	consumed_messages: usize,
	///Accumulated delay of al messages. From message creation (in traffic.rs) to server consumption.
	total_message_delay: usize,
	///Accumulated network delay for all packets. From the leading phit being inserted into a router to the consumption of the tail phit.
	total_packet_network_delay: usize,
	///Accumulated count of hops made for all consumed packets.
	total_packet_hops: usize,
	///Count of consumed packets indexed by the number of hops it made.
	total_packet_per_hop_count: Vec<usize>,
}

impl StatisticMeasurement
{
	//TODO: this do not use `self`, and does not work with temporal statistics.
	fn jain_server_created_phits(&self, network:&Network) -> f64
	{
		//double rcvd_count_total=0.0;
		//double rcvd_count2_total=0.0;
		let mut count=0.0;
		let mut count2=0.0;
		for server in network.servers.iter()
		{
			//double x=(double)(network[i].rcvd_count_from);
			let x=server.statistics.created_phits as f64;
			count+=x;
			count2+=x*x;
		}
		//double Jain_fairness=rcvd_count_total*rcvd_count_total/rcvd_count2_total/(double)nprocs;
		//printf("OUT_Jain_fairness=%f%s",Jain_fairness,sep);
		count*count/count2/network.servers.len() as f64
	}
	fn jain_server_consumed_phits(&self, network:&Network) -> f64
	{
		//double rcvd_count_total=0.0;
		//double rcvd_count2_total=0.0;
		let mut count=0.0;
		let mut count2=0.0;
		for server in network.servers.iter()
		{
			//double x=(double)(network[i].rcvd_count_from);
			let x=server.statistics.consumed_phits as f64;
			count+=x;
			count2+=x*x;
		}
		//double Jain_fairness=rcvd_count_total*rcvd_count_total/rcvd_count2_total/(double)nprocs;
		//printf("OUT_Jain_fairness=%f%s",Jain_fairness,sep);
		count*count/count2/network.servers.len() as f64
	}
}

#[derive(Debug)]
struct StatisticPacketMeasurement
{
	///The cycle in which the packet was consumed, including its tail phit.
	consumed_cycle: usize,
	///The number of router-to-router links the packet has traversed.
	hops: usize,
	///The number of cycles since the packet was created until it was consumed.
	delay: usize,
}

///All the global statistics captured.
#[derive(Debug)]
pub struct Statistics
{
	//The stored path is used for some calls to `config::evaluate`.
	//path: PathBuf,
	///The measurement since the last reset.
	current_measurement: StatisticMeasurement,
	///Specific statistics of the links. Indexed by router and port.
	link_statistics: Vec<Vec<LinkStatistics>>,
	///If non-zero then creates statistics for intervals of the given number of cycles.
	temporal_step: usize,
	///The periodic measurements requested by non-zero statistics_temporal_step.
	temporal_statistics: Vec<StatisticMeasurement>,
	///For each percentile `perc` write server statistics for that percentile. This is, the lowest value such that `perc`% of the servers have lower value.
	///These values will appear in the `server_percentile{perc}` field of the result file.
	///For example, `server_percentile25.injected_load` will be a value with 25% of the servers generating less load and `server_percentile25.accepted_load` will be a value with 25% of the servers consuming less load. Note those values will probably correspond to different servers, despite being written into the same record.
	///The percentiles are integer numbers mostly to make obvious their representation in the name of the field.
	///The default value is empty.
	server_percentiles: Vec<u8>,
	///For each percentile `perc` write packet statistics for that percentile.
	packet_percentiles: Vec<u8>,
	///Data collected to show `packet_percentiles` if not empty.
	packet_statistics: Vec<StatisticPacketMeasurement>,
	///The columns to print in the periodic reports.
	columns: Vec<ReportColumn>,
	///A list of statistic definitions for consumed packets.
	///Each definition is a tuple `(keys,values)`, that are evaluated on each packet.
	///Packets are classified via `keys` into their bin. The number of packets in each bin is counted and the associated `values` are averaged.
	packet_defined_statistics_definitions: Vec< (Vec<Expr>,Vec<Expr>) >,
	///For each definition of packet statistics, we have a vector with an element for each actual value of `keys`.
	///Each of these elements have that value of `key`, together with the averages and the count.
	packet_defined_statistics_measurement: Vec< Vec< (Vec<ConfigurationValue>,Vec<f32>,usize) >>,
}

impl Statistics
{
	fn new(statistics_temporal_step:usize, server_percentiles: Vec<u8>, packet_percentiles: Vec<u8>, statistics_packet_definitions:Vec<(Vec<Expr>,Vec<Expr>)>, topology: &dyn Topology)->Statistics
	{
		let packet_defined_statistics_measurement = vec![ vec![]; statistics_packet_definitions.len() ];
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
			packet_defined_statistics_definitions:statistics_packet_definitions,
			packet_defined_statistics_measurement,
		}
	}
	///Print in stdout a header showing the statistical columns to be periodically printed.
	fn print_header(&self)
	{
		//println!("cycle_begin-cycle_end injected_load accepted_load server_generation_jain_index server_consumption_jain_index");
		let report:String = self.columns.iter().map(|c|c.header()).collect();
		println!("{}",report);
	}
	///Print in stdout the current values of the statistical columns indicated to be periodically printed.
	fn print(&self, next_cycle:usize, network:&Network)
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
	fn reset(&mut self,next_cycle:usize, network:&mut Network)
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
			server.statistics.reset();
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
	fn track_consumed_phit(&mut self, cycle:usize)
	{
		self.current_measurement.consumed_phits+=1;
		if self.temporal_step>0
		{
			let index = cycle / self.temporal_step;
			if self.temporal_statistics.len()<=index
			{
				self.temporal_statistics.resize_with(index+1,Default::default);
				self.temporal_statistics[index].begin_cycle = index*self.temporal_step;
			}
			self.temporal_statistics[index].consumed_phits+=1;
		}
	}
	fn track_consumed_packet(&mut self, cycle:usize, packet:&Packet)
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
		if self.temporal_step>0
		{
			let index = cycle / self.temporal_step;
			if self.temporal_statistics.len()<=index
			{
				self.temporal_statistics.resize_with(index+1,Default::default);
				self.temporal_statistics[index].begin_cycle = index*self.temporal_step;
			}
			self.temporal_statistics[index].consumed_packets+=1;
			self.temporal_statistics[index].total_packet_network_delay+=network_delay;
			self.temporal_statistics[index].total_packet_hops+=hops;
			//Is total_packet_per_hop_count too much here?
		}
		if !self.packet_percentiles.is_empty()
		{
			self.packet_statistics.push(StatisticPacketMeasurement{consumed_cycle:cycle,hops,delay:network_delay});
		}
		if !self.packet_defined_statistics_definitions.is_empty()
		{
			let mut context_content = vec![];
			context_content.push( (String::from("hops"), ConfigurationValue::Number(hops as f64)) );
			context_content.push( (String::from("delay"), ConfigurationValue::Number(network_delay as f64)) );
			context_content.push( (String::from("cycle_into_network"), ConfigurationValue::Number(*packet.cycle_into_network.borrow() as f64)) );
			context_content.push( (String::from("size"), ConfigurationValue::Number(packet.size as f64)) );
			let be = packet.extra.borrow();
			let extra = be.as_ref().unwrap();
			let link_classes = extra.link_classes.iter().map(|x|ConfigurationValue::Number(*x as f64)).collect();
			let entry_virtual_channels = extra.entry_virtual_channels.iter().map(|x|match x{
				Some(v) => ConfigurationValue::Number(*v as f64),
				None => ConfigurationValue::None,
			}).collect();
			let cycle_per_hop = extra.cycle_per_hop.iter().map(|x|ConfigurationValue::Number(*x as f64)).collect();
			context_content.push( (String::from("link_classes"), ConfigurationValue::Array(link_classes)) );
			context_content.push( (String::from("entry_virtual_channels"), ConfigurationValue::Array(entry_virtual_channels)) );
			context_content.push( (String::from("cycle_per_hop"), ConfigurationValue::Array(cycle_per_hop)) );
			let context = ConfigurationValue::Object( String::from("packet"), context_content );
			let path = Path::new(".");
			for (index,definition) in self.packet_defined_statistics_definitions.iter().enumerate()
			{
				let key : Vec<ConfigurationValue> = definition.0.iter().map(|key_expr|config::evaluate( key_expr, &context, path)).collect();
				let value : Vec<f32> = definition.1.iter().map(|key_expr|
					match config::evaluate( key_expr, &context, path){
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
	fn track_consumed_message(&mut self, cycle:usize)
	{
		self.current_measurement.consumed_messages+=1;
		if self.temporal_step>0
		{
			let index = cycle / self.temporal_step;
			if self.temporal_statistics.len()<=index
			{
				self.temporal_statistics.resize_with(index+1,Default::default);
				self.temporal_statistics[index].begin_cycle = index*self.temporal_step;
			}
			self.temporal_statistics[index].consumed_messages+=1;
		}
	}
	fn track_created_phit(&mut self, cycle:usize)
	{
		self.current_measurement.created_phits+=1;
		if self.temporal_step>0
		{
			let index = cycle / self.temporal_step;
			if self.temporal_statistics.len()<=index
			{
				self.temporal_statistics.resize_with(index+1,Default::default);
				self.temporal_statistics[index].begin_cycle = index*self.temporal_step;
			}
			self.temporal_statistics[index].created_phits+=1;
		}
	}
	fn track_message_delay(&mut self, delay:usize, cycle:usize)
	{
		self.current_measurement.total_message_delay+= delay;
		if self.temporal_step>0
		{
			let index = cycle / self.temporal_step;
			if self.temporal_statistics.len()<=index
			{
				self.temporal_statistics.resize_with(index+1,Default::default);
				self.temporal_statistics[index].begin_cycle = index*self.temporal_step;
			}
			self.temporal_statistics[index].total_message_delay+=delay;
		}
	}
	//fn track_packet_hops(&mut self, hops:usize, cycle:usize)
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
}

///The available statistical columns. Each column has a string for the header and a way to compute what to print each period.
#[derive(Debug)]
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
#[derive(Debug)]
struct ReportColumn
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
	fn format(&self, statistics: &Statistics, next_cycle:usize, network:&Network) -> String
	{
		let cycles=next_cycle-statistics.current_measurement.begin_cycle+1;
		let value = match self.kind
		{
			ReportColumnKind::BeginEndCycle => format!("{:>11}-{}",statistics.current_measurement.begin_cycle,next_cycle-1),
			ReportColumnKind::InjectedLoad => format!{"{}",statistics.current_measurement.created_phits as f32/cycles as f32/network.servers.len() as f32},
			ReportColumnKind::AcceptedLoad =>  format!{"{}",statistics.current_measurement.consumed_phits as f32/cycles as f32/network.servers.len() as f32},
			ReportColumnKind::ServerGenerationJainIndex => format!{"{}",statistics.current_measurement.jain_server_created_phits(network)},
			ReportColumnKind::ServerConsumptionJainIndex => format!{"{}",statistics.current_measurement.jain_server_consumed_phits(network)},
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

///The object represeting the whole simulation.
pub struct Simulation<'a>
{
	///The whole parsed configuration.
	#[allow(dead_code)]
	pub configuration: ConfigurationValue,
	///The seed of the random number generator.
	#[allow(dead_code)]
	pub seed: usize,
	///The random number generator itself, with its current state.
	pub rng: RefCell<StdRng>,
	///Cycles of preparation before the actual measured execution
	pub warmup: usize,
	///Cycles of measurement
	pub measured: usize,
	///The instantiated network. It constains the routers and servers connected according to the topology.
	pub network: Network,
	///The traffic being generated/consumed by the servers.
	pub traffic: Box<dyn Traffic>,
	///The maximum size in phits that network packets can have. Any message greater than this is broken into several packets.
	pub maximum_packet_size: usize,
	///The routing algorithm that the network router will employ to set candidate routes.
	pub routing: Box<dyn Routing>,
	///The properties associated to each link class.
	pub link_classes: Vec<LinkClass>,
	///Maximum number of messages for generation to store in each server. Its default value is 20 messages.
	///Attemps to generate traffic that fails because of the limit are tracked into the `missed_generations` statistic.
	///Note that packets are not generated until it is the turn for the message to be sent to a router.
	pub server_queue_size: usize,
	///The queue of events guiding the simulation.
	pub event_queue: EventQueue,
	///The current cycle, i.e, the current discrete time.
	pub cycle:usize,
	///The statistics being collected.
	pub statistics: Statistics,
	///Information abut how to launch simulations to different systems.
	#[allow(dead_code)]
	pub launch_configurations: Vec<ConfigurationValue>,
	///Plugged functions to build traffics, routers, etc.
	pub plugs: &'a Plugs,
}

impl<'a> Simulation<'a>
{
	fn new(cv: &ConfigurationValue, plugs:&'a Plugs) -> Simulation<'a>
	{
		let mut seed: Option<usize> = None;
		let mut topology =None;
		let mut traffic =None;
		let mut router_cfg: Option<&ConfigurationValue> =None;
		let mut warmup = None;
		let mut measured = None;
		let mut maximum_packet_size=None;
		let mut routing=None;
		let mut link_classes = None;
		let mut statistics_temporal_step = 0;
		let mut launch_configurations: Vec<ConfigurationValue> = vec![];
		let mut statistics_server_percentiles: Vec<u8> = vec![];
		let mut statistics_packet_percentiles: Vec<u8> = vec![];
		let mut statistics_packet_definitions:Vec< (Vec<Expr>,Vec<Expr>) > = vec![];
		let mut server_queue_size = None;
		match_object_panic!(cv,"Configuration",value,
			"random_seed" => seed=Some(value.as_f64().expect("bad value for random_seed") as usize),
			"warmup" => warmup=Some(value.as_f64().expect("bad value for warmup") as usize),
			"measured" => measured=Some(value.as_f64().expect("bad value for measured") as usize),
			"topology" => topology=Some(value),
			"traffic" => traffic=Some(value),
			"maximum_packet_size" => maximum_packet_size=Some(value.as_f64().expect("bad value for maximum_packet_size") as usize),
			"server_queue_size" => server_queue_size=Some(value.as_f64().expect("bad value for server_queue_size") as usize),
			"router" => router_cfg=Some(&value),
			"routing" => routing=Some(new_routing(RoutingBuilderArgument{cv:value,plugs})),
			"link_classes" => link_classes = Some(value.as_array().expect("bad value for link_classes").iter()
				.map(|v|LinkClass::new(v)).collect()),
			"statistics_temporal_step" => statistics_temporal_step=value.as_f64().expect("bad value for statistics_temporal_step") as usize,
			"launch_configurations" => launch_configurations = value.as_array().expect("bad value for launch_configurations").clone(),
			"statistics_server_percentiles" => statistics_server_percentiles = value
				.as_array().expect("bad value for statistics_server_percentiles").iter()
				.map(|v|v.as_f64().expect("bad value in statistics_server_percentiles").round() as u8).collect(),
			"statistics_packet_percentiles" => statistics_packet_percentiles = value
				.as_array().expect("bad value for statistics_packet_percentiles").iter()
				.map(|v|v.as_f64().expect("bad value in statistics_packet_percentiles").round() as u8).collect(),
			"statistics_packet_definitions" => match value
			{
				&ConfigurationValue::Array(ref l) => statistics_packet_definitions=l.iter().map(|definition|match definition {
					&ConfigurationValue::Array(ref dl) => {
						if dl.len()!=2
						{
							panic!("Each definition of statistics_packet_definitions must be composed of [keys,values]");
						}
						let keys = match dl[0]
						{
							ConfigurationValue::Array(ref lx) => lx.iter().map(|x|match x{
								ConfigurationValue::Expression(expr) => expr.clone(),
								_ => panic!("bad value for statistics_packet_definitions"),
								}).collect(),
							_ => panic!("bad value for statistics_packet_definitions"),
						};
						let values = match dl[1]
						{
							ConfigurationValue::Array(ref lx) => lx.iter().map(|x|match x{
								ConfigurationValue::Expression(expr) => expr.clone(),
								_ => panic!("bad value for statistics_packet_definitions"),
								}).collect(),
							_ => panic!("bad value for statistics_packet_definitions"),
						};
						(keys,values)
					},
					_ => panic!("bad value for statistics_packet_definitions"),
				}).collect(),
				_ => panic!("bad value for statistics_packet_definitions"),
			}
		);
		let seed=seed.expect("There were no random_seed");
		let warmup=warmup.expect("There were no warmup");
		let measured=measured.expect("There were no measured");
		let topology=topology.expect("There were no topology");
		let traffic=traffic.expect("There were no traffic");
		let maximum_packet_size=maximum_packet_size.expect("There were no maximum_packet_size");
		let server_queue_size = server_queue_size.unwrap_or(20);
		assert!(server_queue_size>0, "we need space in the servers to store generated messages.");
		let router_cfg=router_cfg.expect("There were no router");
		let mut routing=routing.expect("There were no routing");
		let link_classes:Vec<LinkClass>=link_classes.expect("There were no link_classes");
		//This has been changed from rand-0.4 to rand-0.8
		let rng=RefCell::new(StdRng::seed_from_u64(seed as u64));
		let topology=new_topology(TopologyBuilderArgument{
			cv:topology,
			plugs,
			rng:&rng,
		});
		topology.check_adjacency_consistency(Some(link_classes.len()));
		routing.initialize(&topology,&rng);
		let num_routers=topology.num_routers();
		let num_servers=topology.num_servers();
		//let routers: Vec<Rc<RefCell<dyn Router>>>=(0..num_routers).map(|index|new_router(index,router_cfg,plugs,topology.as_ref(),maximum_packet_size)).collect();
		let routers: Vec<Rc<RefCell<dyn Router>>>=(0..num_routers).map(|index|new_router(router::RouterBuilderArgument{
			router_index:index,
			cv:router_cfg,
			plugs,
			topology:topology.as_ref(),
			maximum_packet_size,
			statistics_temporal_step,
		})).collect();
		let servers=(0..num_servers).map(|index|{
			let port=topology.server_neighbour(index);
			let router_status=match port.0
			{
				Location::RouterPort{
					router_index,
					router_port
				} => {
					let router=routers[router_index].borrow();
					let nvc=router.num_virtual_channels();
					let buffer_amount=nvc;
					//TODO: this seems that should a function of the TransmissionFromServer...
					let buffer_size=(0..nvc).map(|vc|router.virtual_port_size(router_port,vc)).max().expect("0 buffers in the router");
					let size_to_send=maximum_packet_size;
					let from_server_mechanism = TransmissionFromServer::new(buffer_amount,buffer_size,size_to_send);
					from_server_mechanism.new_status_at_emissor()
				}
				_ => panic!("Server is not connected to router"),
			};
			Server{
				index,
				port,
				router_status,
				stored_messages:VecDeque::new(),
				stored_packets:VecDeque::new(),
				stored_phits:VecDeque::new(),
				consumed_phits: BTreeMap::new(),
				statistics: ServerStatistics::new(),
			}
		}).collect();
		let traffic=new_traffic(TrafficBuilderArgument{
			cv:traffic,
			plugs,
			topology:&topology,
			rng:&rng,
		});
		let statistics=Statistics::new(statistics_temporal_step,statistics_server_percentiles,statistics_packet_percentiles,statistics_packet_definitions,topology.as_ref());
		Simulation{
			configuration: cv.clone(),
			seed,
			rng,
			warmup,
			measured,
			network: Network{
				topology,
				routers,
				servers,
			},
			traffic,
			maximum_packet_size,
			routing,
			link_classes,
			server_queue_size,
			event_queue: EventQueue::new(1000),
			cycle:0,
			statistics,
			launch_configurations,
			plugs,
		}
	}
	///Run the simulations until it finishes.
	fn run(&mut self)
	{
		self.print_memory_breakdown();
		self.statistics.print_header();
		while self.cycle < self.warmup+self.measured
		{
			self.advance();
			if self.cycle==self.warmup
			{
				self.statistics.reset(self.cycle,&mut self.network);
				self.routing.reset_statistics(self.cycle);
			}
			if self.traffic.is_finished()
			{
				println!("Traffic consumed before cycle {}",self.cycle);
				break;
			}
		}
	}
	///Execute a single cycle of the simulation.
	fn advance(&mut self)
	{
		let mut ievent=0;
		//println!("Begin advance");
		//while let Some(event) = self.event_queue.access_begin(ievent)
		loop
		{
			let event=if let Some(event) = self.event_queue.access_begin(ievent)
			{
				event.clone()
			}
			else
			{
				break;
			};
			//if self.cycle>=3122
			//{
			//	println!("Processing begin event at position {}",ievent);
			//}
			match event
			{
				Event::PhitToLocation{
					ref phit,
					ref previous,
					//router,
					//port,
					ref new,
				} =>
				{
					match new
					{
						&Location::RouterPort{router_index:router,router_port:port} =>
						{
							self.statistics.link_statistics[router][port].phit_arrivals+=1;
							if phit.is_begin() && !self.statistics.packet_defined_statistics_definitions.is_empty()
							{
								let mut be = phit.packet.extra.borrow_mut();
								if let None = *be
								{
									*be=Some(PacketExtraInfo::default());
								}
								let extra = be.as_mut().unwrap();
								let (_,link_class) = self.network.topology.neighbour(router,port);
								extra.link_classes.push(link_class);
								extra.entry_virtual_channels.push(*phit.virtual_channel.borrow());
								extra.cycle_per_hop.push(self.cycle);
							}
							let mut brouter=self.network.routers[router].borrow_mut();
							brouter.insert(phit.clone(),port,&self.rng);
							if brouter.pending_events()==0
							{
								brouter.add_pending_event();
								//self.event_queue.enqueue_end(Event::Generic(self.network.routers[router]),0);
								//self.event_queue.enqueue_end(Event::Generic(self.network.routers[router] as Rc<RefCell<Eventful>>),0);
								self.event_queue.enqueue_end(Event::Generic(brouter.as_eventful().upgrade().expect("missing router")),0);
							}
							match previous
							{
								&Location::ServerPort(_server_index) => if phit.is_begin()
								{
									*phit.packet.cycle_into_network.borrow_mut() = self.cycle;
									self.routing.initialize_routing_info(&phit.packet.routing_info, self.network.topology.as_ref(), router, phit.packet.message.destination,&self.rng);
								},
								&Location::RouterPort{../*router_index,router_port*/} => if phit.is_begin()
								{
									phit.packet.routing_info.borrow_mut().hops+=1;
									self.routing.update_routing_info(&phit.packet.routing_info, self.network.topology.as_ref(), router, port, phit.packet.message.destination,&self.rng);
								},
								_ => (),
							};
						},
						&Location::ServerPort(server) =>
						{
							if server!=phit.packet.message.destination
							{
								panic!("Packet reached wrong server, {} instead of {}!\n",server,phit.packet.message.destination);
							}
							self.network.servers[server].consume(phit.clone(),self.traffic.deref_mut(),&mut self.statistics,self.cycle,&self.network.topology,&self.rng);
						}
						&Location::None => panic!("Phit went nowhere previous={:?}",previous),
					};
				},
				//Event::PhitClearAcknowledge
				Event::Acknowledge{
					location,
					//virtual_channel,
					message: ack_message,
				} => match location
				{
					Location::RouterPort{
						router_index,
						router_port,
					} =>
					{
						let mut brouter=self.network.routers[router_index].borrow_mut();
						//brouter.acknowledge(router_port,virtual_channel);
						brouter.acknowledge(router_port,ack_message);
						if brouter.pending_events()==0
						{
							brouter.add_pending_event();
							self.event_queue.enqueue_end(Event::Generic(brouter.as_eventful().upgrade().expect("missing router")),0);
						}
					},
					Location::ServerPort(server) => self.network.servers[server].router_status.acknowledge(ack_message),
					//&Location::ServerPort(server) => TransmissionFromServer::acknowledge(self.network.servers[server].router_status,ack_message),
					_ => (),
				},
				Event::Generic(ref _element) => unimplemented!(),
			};
			ievent+=1;
		}
		//println!("Done cycle-begin events");
		ievent=0;
		//while let Some(event) = self.event_queue.access_end(ievent)
		loop
		{
			let event=if let Some(event) = self.event_queue.access_end(ievent)
			{
				event.clone()
			}
			else
			{
				break;
			};
			//if self.cycle>=3122
			//{
			//	println!("Processing end event at position {}",ievent);
			//}
			match event
			{
				Event::PhitToLocation{
					..
					//ref phit,
					//ref previous,
					//ref new,
				} => panic!("Phits should not arrive at the end of a cycle"),
				//Event::PhitClearAcknowledge
				Event::Acknowledge{
					..
					//ref location,
					//virtual_channel,
				} => panic!("Phit Acknowledgements should not arrive at the end of a cycle"),
				Event::Generic(ref element) =>
				{
					let new_events=element.borrow_mut().process(self);
					//element.borrow_mut().clear_pending_events();//now done by process itself
					for ge in new_events.into_iter()
					{
						self.event_queue.enqueue(ge);
					}
				},
			};
			ievent+=1;
		}
		//println!("Done cycle-end events");
		let num_servers=self.network.servers.len();
		for (iserver,server) in self.network.servers.iter_mut().enumerate()
		{
			//println!("credits of {} = {}",iserver,server.credits);
			if let (Location::RouterPort{router_index: index,router_port: port},link_class)=server.port
			{
				if self.traffic.should_generate(iserver,self.cycle,&self.rng)
				{
					if server.stored_messages.len()<self.server_queue_size {
						match self.traffic.generate_message(iserver,self.cycle,&self.network.topology,&self.rng)
						{
							Ok(message) =>
							{
								if message.destination>=num_servers
								{
									panic!("Message sent to outside the network unexpectedly.");
								}
								if message.destination==iserver
								{
									panic!("Generated message to self unexpectedly.");
								}
								server.stored_messages.push_back(message);
							},
							Err(TrafficError::OriginOutsideTraffic) => (),
							Err(TrafficError::SelfMessage) => (),
							//Err(error) => panic!("An error happened when generating traffic: {:?}",error),
						};
					} else {
						//There is no space in the server queue of messages.
						server.statistics.missed_generations += 1;
					}
				}
				if server.stored_packets.len()==0 && server.stored_messages.len()>0
				{
					let message=server.stored_messages.pop_front().expect("There are not messages in queue");
					let mut size=message.size;
					while size>0
					{
						let ps=if size>self.maximum_packet_size
						{
							self.maximum_packet_size
						}
						else
						{
							size
						};
						server.stored_packets.push_back(Rc::new(Packet{
							size:ps,
							routing_info: RefCell::new(RoutingInfo::new()),
							message:message.clone(),
							index:0,
							cycle_into_network:RefCell::new(0),
							extra: RefCell::new(None),
						}));
						size-=ps;
					}
				}
				if server.stored_phits.len()==0 && server.stored_packets.len()>0
				{
					let packet=server.stored_packets.pop_front().expect("There are not packets in queue");
					for index in 0..packet.size
					{
						server.stored_phits.push_back(Rc::new(Phit{
							packet:packet.clone(),
							index,
							virtual_channel: RefCell::new(None),
						}));
					}
				}
				//if server.stored_phits.len()>0 && server.credits>0
				//{
				//	let phit=server.stored_phits.pop_front().expect("There are not phits");
				//	let event=Event::PhitToLocation{
				//		phit,
				//		previous: Location::ServerPort(iserver),
				//		new: Location::RouterPort{router_index:index,router_port:port},
				//	};
				//	self.statistics.created_phits+=1;
				//	server.statistics.created_phits+=1;
				//	self.event_queue.enqueue_begin(event,self.link_classes[link_class].delay);
				//	server.credits-=1;
				//}
				if server.stored_phits.len()>0
				{
					//Do not extract the phit until we know whether we can transmit it.
					let phit=server.stored_phits.front().expect("There are not phits");
					if server.router_status.can_transmit(&phit,0)
					{
						let phit=server.stored_phits.pop_front().expect("There are not phits");
						let event=Event::PhitToLocation{
							phit,
							previous: Location::ServerPort(iserver),
							new: Location::RouterPort{router_index:index,router_port:port},
						};
						//self.statistics.created_phits+=1;
						self.statistics.track_created_phit(self.cycle);
						server.statistics.created_phits+=1;
						server.statistics.cycle_last_created_phit = self.cycle;
						self.event_queue.enqueue_begin(event,self.link_classes[link_class].delay);
						server.router_status.notify_outcoming_phit(0,self.cycle);
					}
				}
			}
			else
			{
				panic!("Where goes this port?");
			}
		}
		//println!("Done generation");
		//if self.cycle%1000==999
		//{
		//	self.print_memory_breakdown();
		//}
		self.event_queue.advance();
		self.cycle+=1;
		if self.cycle%1000==0
		{
			//println!("Statistics up to cycle {}: {:?}",self.cycle,self.statistics);
			self.statistics.print(self.cycle,&self.network);
			//self.print_memory_breakdown();
		}
	}
	///Write the result of the simulation somewhere, typically to a 'result' file in a 'run*' directory.
	fn write_result(&self,output:&mut dyn Write)
	{
		// https://stackoverflow.com/questions/22355273/writing-to-a-file-or-stdout-in-rust
		//output.write(b"Hello from the simulator\n").unwrap();
		//Result
		//{
		//	accepted_load: 0.9,
		//	average_message_delay: 100,
		//}
		let measurement = &self.statistics.current_measurement;
		let cycles=self.cycle-measurement.begin_cycle;
		let num_servers=self.network.servers.len();
		let injected_load=measurement.created_phits as f64/cycles as f64/num_servers as f64;
		let accepted_load=measurement.consumed_phits as f64/cycles as f64/num_servers as f64;
		let average_message_delay=measurement.total_message_delay as f64/measurement.consumed_messages as f64;
		let average_packet_network_delay=measurement.total_packet_network_delay as f64/measurement.consumed_packets as f64;
		let jscp=measurement.jain_server_consumed_phits(&self.network);
		let jsgp=measurement.jain_server_created_phits(&self.network);
		let average_packet_hops=measurement.total_packet_hops as f64 / measurement.consumed_packets as f64;
		let total_packet_per_hop_count=measurement.total_packet_per_hop_count.iter().map(|&count|ConfigurationValue::Number(count as f64)).collect();
		//let total_arrivals:usize = self.statistics.link_statistics.iter().map(|rls|rls.iter().map(|ls|ls.phit_arrivals).sum::<usize>()).sum();
		//let total_links:usize = self.statistics.link_statistics.iter().map(|rls|rls.len()).sum();
		let total_arrivals:usize = (0..self.network.topology.num_routers()).map(|i|(0..self.network.topology.degree(i)).map(|j|self.statistics.link_statistics[i][j].phit_arrivals).sum::<usize>()).sum();
		let total_links: usize = (0..self.network.topology.num_routers()).map(|i|self.network.topology.degree(i)).sum();
		let average_link_utilization = total_arrivals as f64 / cycles as f64 / total_links as f64;
		let maximum_arrivals:usize = self.statistics.link_statistics.iter().map(|rls|rls.iter().map(|ls|ls.phit_arrivals).max().unwrap()).max().unwrap();
		let maximum_link_utilization = maximum_arrivals as f64 / cycles as f64;
		let server_average_cycle_last_created_phit : f64 = (self.network.servers.iter().map(|s|s.statistics.cycle_last_created_phit).sum::<usize>() as f64)/(self.network.servers.len() as f64);
		let server_average_cycle_last_consumed_message : f64 = (self.network.servers.iter().map(|s|s.statistics.cycle_last_consumed_message).sum::<usize>() as f64)/(self.network.servers.len() as f64);
		let server_average_missed_generations : f64 = (self.network.servers.iter().map(|s|s.statistics.missed_generations).sum::<usize>() as f64)/(self.network.servers.len() as f64);
		let servers_with_missed_generations : usize = self.network.servers.iter().map(|s|if s.statistics.missed_generations > 0 {1} else {0}).sum::<usize>();
		let git_id=get_git_id();
		let version_number = get_version_number();
		let mut result_content = vec![
			(String::from("cycle"),ConfigurationValue::Number(self.cycle as f64)),
			(String::from("injected_load"),ConfigurationValue::Number(injected_load)),
			(String::from("accepted_load"),ConfigurationValue::Number(accepted_load)),
			(String::from("average_message_delay"),ConfigurationValue::Number(average_message_delay)),
			(String::from("average_packet_network_delay"),ConfigurationValue::Number(average_packet_network_delay)),
			(String::from("server_generation_jain_index"),ConfigurationValue::Number(jsgp)),
			(String::from("server_consumption_jain_index"),ConfigurationValue::Number(jscp)),
			(String::from("average_packet_hops"),ConfigurationValue::Number(average_packet_hops)),
			(String::from("total_packet_per_hop_count"),ConfigurationValue::Array(total_packet_per_hop_count)),
			(String::from("average_link_utilization"),ConfigurationValue::Number(average_link_utilization)),
			(String::from("maximum_link_utilization"),ConfigurationValue::Number(maximum_link_utilization)),
			(String::from("server_average_cycle_last_created_phit"),ConfigurationValue::Number(server_average_cycle_last_created_phit)),
			(String::from("server_average_cycle_last_consumed_message"),ConfigurationValue::Number(server_average_cycle_last_consumed_message)),
			(String::from("server_average_missed_generations"),ConfigurationValue::Number(server_average_missed_generations)),
			(String::from("servers_with_missed_generations"),ConfigurationValue::Number(servers_with_missed_generations as f64)),
			//(String::from("git_id"),ConfigurationValue::Literal(format!("\"{}\"",git_id))),
			(String::from("git_id"),ConfigurationValue::Literal(format!("{}",git_id))),
			(String::from("version_number"),ConfigurationValue::Literal(format!("{}",version_number))),
		];
		if let Some(content)=self.routing.statistics(self.cycle)
		{
			result_content.push((String::from("routing_statistics"),content));
		}
		if let Some(content) = self.network.routers.iter().enumerate().fold(None,|maybe_stat,(index,router)|router.borrow().aggregate_statistics(maybe_stat,index,self.network.routers.len(),self.cycle))
		{
			result_content.push((String::from("router_aggregated_statistics"),content));
		}
		if let Ok(linux_process) = procfs::process::Process::myself()
		{
			let status = linux_process.status().expect("failed to get status of the self process");
			if let Some(peak_memory)=status.vmhwm
			{
				//Peak resident set size by kibibytes ("high water mark").
				result_content.push((String::from("linux_high_water_mark"),ConfigurationValue::Number(peak_memory as f64)));
			}
			let stat = linux_process.stat().expect("failed to get stat of the self process");
			let tps = procfs::ticks_per_second().expect("could not get the number of ticks per second.") as f64;
			result_content.push((String::from("user_time"),ConfigurationValue::Number(stat.utime as f64/tps)));
			result_content.push((String::from("system_time"),ConfigurationValue::Number(stat.stime as f64/tps)));
		}
		if self.statistics.temporal_step > 0
		{
			let step = self.statistics.temporal_step;
			let samples = self.statistics.temporal_statistics.len();
			let mut injected_load_collect = Vec::with_capacity(samples);
			let mut accepted_load_collect = Vec::with_capacity(samples);
			let mut average_message_delay_collect = Vec::with_capacity(samples);
			let mut average_packet_network_delay_collect = Vec::with_capacity(samples);
			let mut jscp_collect = Vec::with_capacity(samples);
			let mut jsgp_collect = Vec::with_capacity(samples);
			let mut average_packet_hops_collect = Vec::with_capacity(samples);
			for measurement in self.statistics.temporal_statistics.iter()
			{
				let injected_load=measurement.created_phits as f64/step as f64/num_servers as f64;
				injected_load_collect.push(ConfigurationValue::Number(injected_load));
				let accepted_load=measurement.consumed_phits as f64/step as f64/num_servers as f64;
				accepted_load_collect.push(ConfigurationValue::Number(accepted_load));
				let average_message_delay=measurement.total_message_delay as f64/measurement.consumed_messages as f64;
				average_message_delay_collect.push(ConfigurationValue::Number(average_message_delay));
				let average_packet_network_delay=measurement.total_message_delay as f64/measurement.consumed_messages as f64;
				average_packet_network_delay_collect.push(ConfigurationValue::Number(average_packet_network_delay));
				let jscp=measurement.jain_server_consumed_phits(&self.network);
				jscp_collect.push(ConfigurationValue::Number(jscp));
				let jsgp=measurement.jain_server_created_phits(&self.network);
				jsgp_collect.push(ConfigurationValue::Number(jsgp));
				let average_packet_hops=measurement.total_packet_hops as f64 / measurement.consumed_packets as f64;
				average_packet_hops_collect.push(ConfigurationValue::Number(average_packet_hops));
			};
			let temporal_content = vec![
				//(String::from("cycle"),ConfigurationValue::Number(self.cycle as f64)),
				(String::from("injected_load"),ConfigurationValue::Array(injected_load_collect)),
				(String::from("accepted_load"),ConfigurationValue::Array(accepted_load_collect)),
				(String::from("average_message_delay"),ConfigurationValue::Array(average_message_delay_collect)),
				(String::from("average_packet_network_delay"),ConfigurationValue::Array(average_packet_network_delay_collect)),
				(String::from("server_generation_jain_index"),ConfigurationValue::Array(jsgp_collect)),
				(String::from("server_consumption_jain_index"),ConfigurationValue::Array(jscp_collect)),
				(String::from("average_packet_hops"),ConfigurationValue::Array(average_packet_hops_collect)),
				//(String::from("total_packet_per_hop_count"),ConfigurationValue::Array(total_packet_per_hop_count)),
				//(String::from("average_link_utilization"),ConfigurationValue::Number(average_link_utilization)),
				//(String::from("maximum_link_utilization"),ConfigurationValue::Number(maximum_link_utilization)),
				//(String::from("git_id"),ConfigurationValue::Literal(format!("{}",git_id))),
			];
			result_content.push((String::from("temporal_statistics"),ConfigurationValue::Object(String::from("TemporalStatistics"),temporal_content)));
		}
		if !self.statistics.server_percentiles.is_empty()
		{
			let mut servers_injected_load : Vec<f64> = self.network.servers.iter().map(|s|s.statistics.created_phits as f64/cycles as f64).collect();
			let mut servers_accepted_load : Vec<f64> = self.network.servers.iter().map(|s|s.statistics.consumed_phits as f64/cycles as f64).collect();
			let mut servers_average_message_delay : Vec<f64> = self.network.servers.iter().map(|s|s.statistics.total_message_delay as f64/s.statistics.consumed_messages as f64).collect();
			let mut servers_cycle_last_created_phit : Vec<usize> = self.network.servers.iter().map(|s|s.statistics.cycle_last_created_phit).collect();
			let mut servers_cycle_last_consumed_message : Vec<usize> = self.network.servers.iter().map(|s|s.statistics.cycle_last_consumed_message).collect();
			let mut servers_missed_generations : Vec<usize> = self.network.servers.iter().map(|s|s.statistics.missed_generations).collect();
			//XXX There are more efficient ways to find percentiles than to sort them, but should not be notable in any case. See https://en.wikipedia.org/wiki/Selection_algorithm
			servers_injected_load.sort_by(|a,b|a.partial_cmp(b).unwrap_or(Ordering::Less));
			servers_accepted_load.sort_by(|a,b|a.partial_cmp(b).unwrap_or(Ordering::Less));
			servers_average_message_delay.sort_by(|a,b|a.partial_cmp(b).unwrap_or(Ordering::Less));
			servers_cycle_last_created_phit.sort();
			servers_cycle_last_consumed_message.sort();
			servers_missed_generations.sort();
			for &percentile in self.statistics.server_percentiles.iter()
			{
				let mut index:usize = num_servers * usize::from(percentile) /100;
				if index >= num_servers
				{
					//This happens at least in percentile 100%.
					//We cannot find a value greater than ALL, just return the greatest.
					index = num_servers -1;
				}
				let server_content = vec![
					(String::from("injected_load"),ConfigurationValue::Number(servers_injected_load[index])),
					(String::from("accepted_load"),ConfigurationValue::Number(servers_accepted_load[index])),
					(String::from("average_message_delay"),ConfigurationValue::Number(servers_average_message_delay[index])),
					(String::from("cycle_last_created_phit"),ConfigurationValue::Number(servers_cycle_last_created_phit[index] as f64)),
					(String::from("cycle_last_consumed_message"),ConfigurationValue::Number(servers_cycle_last_consumed_message[index] as f64)),
					(String::from("missed_generations"),ConfigurationValue::Number(servers_missed_generations[index] as f64)),
				];
				result_content.push((format!("server_percentile{}",percentile),ConfigurationValue::Object(String::from("ServerStatistics"),server_content)));
			}
		}
		if !self.statistics.packet_percentiles.is_empty()
		{
			let mut packets_delay : Vec<usize> = self.statistics.packet_statistics.iter().map(|ps|ps.delay).collect();
			let mut packets_hops : Vec<usize> = self.statistics.packet_statistics.iter().map(|ps|ps.hops).collect();
			let mut packets_consumed_cycle: Vec<usize> = self.statistics.packet_statistics.iter().map(|ps|ps.consumed_cycle).collect();
			packets_delay.sort();
			packets_hops.sort();
			packets_consumed_cycle.sort();
			let num_packets = packets_delay.len();
			for &percentile in self.statistics.packet_percentiles.iter()
			{
				let mut index:usize = num_packets * usize::from(percentile) /100;
				if index >= num_packets
				{
					//This happens at least in percentile 100%.
					//We cannot find a value greater than ALL, just return the greatest.
					index = num_packets -1;
				}
				let packet_content = vec![
					(String::from("delay"),ConfigurationValue::Number(packets_delay[index] as f64)),
					(String::from("hops"),ConfigurationValue::Number(packets_hops[index] as f64)),
					(String::from("consumed_cycle"),ConfigurationValue::Number(packets_consumed_cycle[index] as f64)),
				];
				result_content.push((format!("packet_percentile{}",percentile),ConfigurationValue::Object(String::from("PacketStatistics"),packet_content)));
			}
		}
		if !self.statistics.packet_defined_statistics_measurement.is_empty()
		{
			let mut pds_content=vec![];
			for definition_measurement in self.statistics.packet_defined_statistics_measurement.iter()
			{
				let mut dm_list = vec![];
				for (key,val,count) in definition_measurement
				{
					let fcount = *count as f32;
					//One average for each value field
					let averages = ConfigurationValue::Array( val.iter().map(|v|ConfigurationValue::Number(f64::from(v/fcount))).collect() );
					let dm_content: Vec<(String,ConfigurationValue)> = vec![
						(String::from("key"),ConfigurationValue::Array(key.to_vec())),
						(String::from("average"),averages),
						(String::from("count"),ConfigurationValue::Number(*count as f64)),
					];
					dm_list.push( ConfigurationValue::Object(String::from("PacketBin"),dm_content) );
				}
				pds_content.push(ConfigurationValue::Array(dm_list));
			}
			result_content.push( (String::from("packet_defined_statistics"),ConfigurationValue::Array(pds_content)) );
		}
		let result=ConfigurationValue::Object(String::from("Result"),result_content);
		writeln!(output,"{}",result).unwrap();
	}
}

impl<'a> Quantifiable for Simulation<'a>
{
	fn total_memory(&self) -> usize
	{
		unimplemented!();
	}
	fn print_memory_breakdown(&self)
	{
		println!("\nBegin memory report");
		println!("self : {}",size_of::<Self>());
		//println!("phits on statistics : {}",self.statistics.created_phits-self.statistics.consumed_phits);
		println!("phit : {}",size_of::<Phit>());
		println!("packet : {}",size_of::<Packet>());
		println!("message : {}",size_of::<Message>());
		//println!("topology : {}",size_of::<dyn Topology>());
		//println!("router : {}",size_of::<dyn Router>());
		println!("server : {}",size_of::<Server>());
		println!("event : {}",size_of::<Event>());
		//self.event_queue.print_memory();
		println!("network total : {}",quantify::human_bytes(self.network.total_memory()));
		println!("traffic total : {}",quantify::human_bytes(self.traffic.total_memory()));
		println!("event_queue total : {}",quantify::human_bytes(self.event_queue.total_memory()));
		//println!("topology total : {}",quantify::human_bytes(self.network.topology.total_memory()));
		println!("End memory report\n");
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}


#[derive(Default)]
pub struct Plugs
{
	//routers: BTreeMap<String, fn(usize,&ConfigurationValue,&Plugs, &dyn Topology, usize) -> Rc<RefCell<dyn Router>>  >,
	routers: BTreeMap<String, fn(RouterBuilderArgument) -> Rc<RefCell<dyn Router>>  >,
	//topologies: BTreeMap<String, fn(&ConfigurationValue, &Plugs, &RefCell<StdRng>) -> Box<dyn Topology> >,
	topologies: BTreeMap<String, fn(TopologyBuilderArgument) -> Box<dyn Topology> >,
	stages: BTreeMap<String, fn(StageBuilderArgument) -> Box<dyn Stage> >,
	//routings: BTreeMap<String,fn(&ConfigurationValue, &Plugs) -> Box<dyn Routing>>,
	routings: BTreeMap<String,fn(RoutingBuilderArgument) -> Box<dyn Routing>>,
	//traffics: BTreeMap<String,fn(&ConfigurationValue, &Plugs, &Box<dyn Topology>, &RefCell<StdRng>) -> Box<dyn Traffic> >,
	traffics: BTreeMap<String,fn(TrafficBuilderArgument) -> Box<dyn Traffic> >,
	patterns: BTreeMap<String, fn(PatternBuilderArgument) -> Box<dyn Pattern> >,
	policies: BTreeMap<String, fn(VCPolicyBuilderArgument) -> Box<dyn VirtualChannelPolicy> >,
}

impl Plugs
{
	//pub fn add_router(&mut self, key:String, builder:fn(usize,&ConfigurationValue,&Plugs, &dyn Topology, usize) -> Rc<RefCell<dyn Router>>)
	pub fn add_router(&mut self, key:String, builder:fn(RouterBuilderArgument) -> Rc<RefCell<dyn Router>>)
	{
		self.routers.insert(key,builder);
	}
	pub fn add_topology(&mut self, key:String, builder:fn(TopologyBuilderArgument) -> Box<dyn Topology>)
	{
		self.topologies.insert(key,builder);
	}
	pub fn add_stage(&mut self, key:String, builder:fn(StageBuilderArgument) -> Box<dyn Stage>)
	{
		self.stages.insert(key,builder);
	}
	pub fn add_traffic(&mut self, key:String, builder:fn(TrafficBuilderArgument) -> Box<dyn Traffic>)
	{
		self.traffics.insert(key,builder);
	}
	pub fn add_routing(&mut self, key:String, builder:fn(RoutingBuilderArgument) -> Box<dyn Routing>)
	{
		self.routings.insert(key,builder);
	}
	pub fn add_policy(&mut self, key:String, builder: fn(VCPolicyBuilderArgument) -> Box<dyn VirtualChannelPolicy>)
	{
		self.policies.insert(key,builder);
	}
	pub fn add_pattern(&mut self, key:String, builder: fn(PatternBuilderArgument) -> Box<dyn Pattern>)
	{
		self.patterns.insert(key,builder);
	}
}

impl Debug for Plugs
{
	fn fmt(&self,f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error>
	{
		write!(f,"{};",self.routers.keys().map(|s|s.to_string()).collect::<Vec<String>>().join(","))?;
		write!(f,"{};",self.topologies.keys().map(|s|s.to_string()).collect::<Vec<String>>().join(","))?;
		write!(f,"{};",self.stages.keys().map(|s|s.to_string()).collect::<Vec<String>>().join(","))?;
		write!(f,"{};",self.routings.keys().map(|s|s.to_string()).collect::<Vec<String>>().join(","))?;
		write!(f,"{};",self.traffics.keys().map(|s|s.to_string()).collect::<Vec<String>>().join(","))?;
		write!(f,"{};",self.patterns.keys().map(|s|s.to_string()).collect::<Vec<String>>().join(","))?;
		write!(f,"{};",self.policies.keys().map(|s|s.to_string()).collect::<Vec<String>>().join(","))?;
		Ok(())
	}
}

/// Main when passed a configuration file as path
/// `file` must be a configuration file with the experiment to simulate.
/// `plugs` constains the plugged builder functions.
/// `result_file` indicates where to write the results.
/// `free_args` are free arguments. Those of the form `path=value` are used to override configurations.
pub fn file_main(file:&mut File, plugs:&Plugs, mut results_file:Option<File>,free_args:&[String])
{
	let mut contents = String::new();
	file.read_to_string(&mut contents).expect("something went wrong reading the file");

	let mut rewrites: Vec< (Expr,ConfigurationValue) > = vec![];
	for arg in free_args
	{
		if let Some( (left,right) ) = arg.split_once('=')
		{
			let left_expr = match config_parser::parse_expression(left).expect("error parsing a free argument")
			{
				config_parser::Token::Expression(expr) => expr,
				x => panic!("the left of free argument is not an expression ({:?}), which it should be.",x),
			};
			//let right_expr = match config_parser::parse_expression(right).expect("error parsing a free argument")
			//{
			//	config_parser::Token::Expression(expr) => expr,
			//	x => panic!("the right of free argument is not an expression ({:?}), which it should be.",x),
			//};
			let right_value = match config_parser::parse(right).expect("error parsing a free argument")
			{
				config_parser::Token::Value(value) => value,
				x => panic!("the right of free argument is not a value ({:?}), which it should be.",x),
			};
			rewrites.push( (left_expr,right_value) );
		} else {
			println!("WARNING: ignoring argument {}",arg);
		}
	}

	//let working_directory=std::env::current_dir().expect("Could not get working directory.");

	//println!("With text:\n{}", contents);
	match config_parser::parse(&contents)
	{
		Err(x) => println!("error parsing configuration file: {:?}",x),
		Ok(mut x) =>
		{
			println!("parsed correctly: {:?}",x);
			match x
			{
				config_parser::Token::Value(ref mut value) =>
				{
					for (path_expr,new_value) in rewrites
					{
						//config::rewrite_pair(value,&path_expr,&new_value,&working_directory);
						config::rewrite_pair_value(value,&path_expr,new_value);
					}
					let flat=flatten_configuration_value(value);
					if let ConfigurationValue::Experiments(ref experiments)=flat
					{
						for (i,experiment) in experiments.iter().enumerate()
						{
							println!("experiment {} of {} is {:?}",i,experiments.len(),experiment);
							let mut simulation=Simulation::new(&experiment,plugs);
							simulation.run();
							match results_file
							{
								Some(ref mut f) => simulation.write_result(f),
								None => simulation.write_result(&mut stdout()),
							};
						}
					}
					else
					{
						panic!("there are not experiments");
					}
				},
				_ => panic!("Not a value"),
			};
		},
	};
}


/// Main when passed a directory as path
/// `path` must be a directory containing a `main.cfg`.
/// `plugs` constains the plugged builder functions.
/// `action` is the action to be performed in the experiment. For example running the simulations or drawing graphics.
/// `options` encapsulate other parameters such as restricting the performed action to a range of simulations.
//pub fn directory_main(path:&Path, binary:&str, plugs:&Plugs, option_matches:&Matches)
pub fn directory_main(path:&Path, binary:&str, plugs:&Plugs, action:Action, options: ExperimentOptions)
{
	if !path.exists()
	{
		println!("Folder {:?} does not exists; creating it.",path);
		fs::create_dir(&path).expect("Something went wrong when creating the main path.");
	}
	let binary_path=Path::new(binary);
	//let mut experiment=Experiment::new(binary_path,path,plugs,option_matches);
	let mut experiment=Experiment::new(binary_path,path,plugs,options);
	//let action=if option_matches.opt_present("action")
	//{
	//	Action::from_str(&option_matches.opt_str("action").unwrap()).expect("Illegal action")
	//}
	//else
	//{
	//	Action::LocalAndOutput
	//};
	match experiment.execute_action(action)
	{
		Ok(()) => (),
		Err(error) =>
		{
			eprintln!("Execution the action {} failed with errors:\n{}",action,error);
		}
	}
	//println!("{:?} is a path",path);
}

/// Get an identifier of the git commit. It is of little use to someone using a forzen public version.
/// The value is fixed in the build script.
pub fn get_git_id() -> &'static str
{
	include_str!(concat!(env!("OUT_DIR"), "/generated_git_id"))
}

/// Get the number currently written in the Cargo.toml field `version`.
/// In public version this is more useful than `get_git_id`.
pub fn get_version_number() -> &'static str
{
	//include_str!(concat!(env!("OUT_DIR"), "/generated_version_number"))
	match option_env!("CARGO_PKG_VERSION")
	{
		Some( version ) => version,
		_ => "?",
	}
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
