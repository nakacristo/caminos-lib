/*!
caminos-lib
=====

This crate provides the CAMINOS simulator as a library. This is the Cantabrian Adaptable and Modular Interconnection Open Simulator.

# Usage

This crate is `caminos-lib`. To use it add `caminos-lib` to your dependencies in your project's `Cargo.toml`.

```toml
[dependencies]
caminos-lib = "0.6"
```

Alternatively, consider whether the binary crate `caminos` fits your intended use.

# Breaking changes

<details>

## [0.6.0] to ...
* All cycles are now represented by a `Time` alias of `u64`; instead of `usize`.
* Removed methods `pending_events`, `add_pending_event`, and `clear_pending_events` from the Eventful trait in favor of the `schedule` method.
* Router methods insert and acknowledge now return `Vec<EventGeneration>` and are responsible for their scheduling.
* Renamed in Traffic nomenclature servers into tasks. This includes ServerTrafficState renamed into TaskTrafficState, and `server_state` into `task_state`. Old configuration names are still supported.
* Added method `number_tasks`required for trait Traffic.

## [0.5.0] to [0.6.0]
* Removed unnecessary generic parameter TM from routers Basic and InputOutput. They now may select [TransmissionMechanisms](router::TransmissionMechanism) to employ.
* Renamed TransmissionFromServer into TransmissionFromOblivious.
* Some changes in the Dragonfly struct, to allow for more global arrangements.
* `Event::process` now receives SimulationShared and SimulationMut for better encapsulation.
* Replaced every `&RefCell<StdRng>` by `&mut StdRng` everywhere.

## [0.4.0] to [0.5.0]

* Added the function `server_state` to the `Traffic` trait.
* Functions on the output module now use ExperimentFiles instead of Path.
* Added a server argument to `Traffic::try_consume`.
* Added phit to `RequestInfo`.
* Upgrade from rand-0.4 to rand-0.8.
* Using `&dyn Topology` instead of `&Box<dyn Topology>` in all interfaces.
* `Topology::coordinated_routing_record` now receives slices.
* `CartesianData::new` now receives an slice.
* SpaceAtReceptor and Stage now uses the Error type in its Result types.
* `config::{evaluate,reevaluate}` now returns a `Result`.

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

```ignore
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

```ignore
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
	traffic: HomogeneousTraffic//Select a traffic. e.g., traffic repeating a pattern continuously.
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
		delay: 0,//not actually implemented in the basic router. In the future it may be removed or actually implemented.
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
		//In a dragonfly topology we would have 0=routers from same group, 1=routers from different groups, and 2=from server
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

An example of output description `main.od` is
```ignore
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

Fot the `tikz` backend to work it is necessary to have a working `LaTeX` installation that includes the `pgfplots` package. It is part of the `texlive-pictures` package in some linux distributions. It may also require the `texlive-latexextra` package.

# Plugging

Both entries `directory_main` and `file_main` receive a `&Plugs` argument that may be used to provide the simulator with new implementations. This way, one can make a copy of the `main` in the `caminos` crate and declare plugs for their implemented `Router`, `Topology`, `Routing`, `Traffic`, `Pattern`, and `VirtualChannelPolicy`.

*/

// --- crate attributes ---
// At clippy::correctness no problem should appear
	// $(cargo clippy -- -A clippy::all -W clippy::correctness)
// At clippy::suspicious
	#![allow(clippy::suspicious_else_formatting)]
// At clippy::style
	// These should be partially addressed, but of very little importance.
	#![allow(clippy::needless_return)]
	#![allow(clippy::new_without_default)]
	#![allow(clippy::comparison_chain)]//is this really clearer???
	#![allow(clippy::single_match)]
	#![allow(clippy::let_and_return)]
	#![allow(clippy::len_without_is_empty)]
	// What is the more appropriate way to iterate a couple arrays of same size, while also using the index itself?
	#![allow(clippy::needless_range_loop)]
	// I have several cases that seem cleaner without collapsing.
	#![allow(clippy::collapsible_else_if)]
	// Ignore these lints
	#![allow(clippy::match_ref_pats)]
	#![allow(clippy::tabs_in_doc_comments)]
// At clippy::complexity
	#![allow(clippy::type_complexity)]
	//I only use this in some cases that would become extremely verbose.
	#![allow(clippy::option_map_unit_fn)]
// At clippy::perf all should be addressed.
// clippy::{pedantic,nursery} seem better to be allowed, as it is the default
// At clippy::cargo
	#![warn(clippy::cargo)]
	//missing repository and categories.
	#![allow(clippy::cargo_common_metadata)]

pub use quantifiable_derive::Quantifiable;//the derive macro

//config_parser contains automatically generated code. No sense in being too strict.
#[allow(clippy::manual_map)]
#[allow(clippy::match_single_binding)]
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
pub mod measures;
pub mod allocator;
pub mod packet;

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
use std::cmp::Ordering;
//use std::default::default;
//use std::borrow::Cow;
use rand::{rngs::StdRng,SeedableRng};

use config_parser::{ConfigurationValue,Expr};
use topology::{Topology,new_topology,TopologyBuilderArgument,Location,
	multistage::{Stage,StageBuilderArgument}};
use traffic::{Traffic,new_traffic,TrafficBuilderArgument,TrafficError};
use router::{Router,new_router,RouterBuilderArgument};
use routing::{RoutingInfo,Routing,new_routing,RoutingBuilderArgument};
use event::{EventQueue,Event,EventGeneration};
use quantify::Quantifiable;
use experiments::{Experiment,Action,ExperimentOptions};
use policies::{VirtualChannelPolicy,VCPolicyBuilderArgument};
use pattern::{Pattern,PatternBuilderArgument};
use config::flatten_configuration_value;
use measures::{Statistics,ServerStatistics};
use error::{Error,SourceLocation};
use allocator::{Allocator,AllocatorBuilderArgument};
pub use packet::{Phit,Packet,Message,PacketExtraInfo,PacketRef,AsMessage};
pub use event::Time;

///The objects that create and consume traffic to/from the network.
#[derive(Quantifiable)]
pub struct Server
{
	///The index of the server in the network.
	index: usize,
	///To which router the server is connected + link class index. Although we could just compute with the topology each time...
	port: (Location,usize),
	///Known available capacity in the connected router.
	router_status: Box<dyn router::StatusAtEmissor+'static>,
	///Created messages but not sent.
	stored_messages: VecDeque<Rc<Message>>,
	///The packets of the message that have not yet been sent.
	stored_packets: VecDeque<PacketRef>,
	///The phits of a packet being sent.
	stored_phits: VecDeque<Rc<Phit>>,
	/// If there is a packet currently being transmitted, then the virtual channel requested if any.
	outcoming_virtual_channel: Option<usize>,
	///For each message we store the number of consumed phits, until the whole message is consumed.
	consumed_phits: BTreeMap<*const Message,usize>,
	///Statistics local to the server.
	statistics: ServerStatistics,
}

impl Server
{
	///Consumes a phit
	fn consume(&mut self, phit:Rc<Phit>, traffic:&mut dyn Traffic, statistics:&mut Statistics, cycle:Time, topology:&dyn Topology, rng: &mut StdRng)
	{
		self.statistics.track_consumed_phit(cycle);
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
			self.statistics.track_consumed_message(cycle);
			statistics.track_consumed_message(cycle);
			self.statistics.track_message_delay(cycle-message.creation_cycle,cycle);
			statistics.track_message_delay(cycle-message.creation_cycle,cycle);
			self.consumed_phits.remove(&message_ptr);
			if !traffic.consume(self.index, &*message, cycle, topology, rng)
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
			statistics.track_consumed_packet(cycle,&phit.packet);
			if cp < phit.packet.size
			{
				println!("phit tail has been consuming without having consumed a whole packet.");
			}
			phit.packet.destroy();//See the notes on the raw_packet feature.
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
	///The topology defining the connectivity.
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

impl Network
{
	fn jain_server_created_phits(&self) -> f64
	{
		measures::jain(self.servers.iter().map(|s|s.statistics.current_measurement.created_phits as f64))
	}
	fn jain_server_consumed_phits(&self) -> f64
	{
		measures::jain(self.servers.iter().map(|s|s.statistics.current_measurement.consumed_phits as f64))
	}
	fn temporal_jain_server_created_phits<'a>(&'a self) -> Vec<f64>
	{
		//measures::jain(self.servers.iter().map(|s|s.statistics.current_measurement.created_phits as f64))
		let limit:usize = self.servers.iter().map(|s|s.statistics.temporal_statistics.len()).max().unwrap_or(0);
		(0..limit).map(|index|{
			measures::jain(self.servers.iter().map(|s|
				s.statistics.temporal_statistics.get(index).map(|m|m.created_phits as f64).unwrap_or(0f64)
			))
		}).collect()
	}
	fn temporal_jain_server_consumed_phits<'a>(&'a self) -> Vec<f64>
	{
		//measures::jain(self.servers.iter().map(|s|s.statistics.current_measurement.consumed_phits as f64))
		let limit:usize = self.servers.iter().map(|s|s.statistics.temporal_statistics.len()).max().unwrap_or(0);
		(0..limit).map(|index|{
			measures::jain(self.servers.iter().map(|s|
				s.statistics.temporal_statistics.get(index).map(|m|m.created_phits as f64).unwrap_or(0f64)
			))
		}).collect()
	}
	fn get_temporal_statistics_servers_expr<'a>(&'a self, temporal_server_defined_statistics_definitions: &Vec< (Vec<Expr>, Vec<Expr>)>) -> Vec< Vec< Vec< (Vec<ConfigurationValue>, Vec<f32>, usize) >>>
	{
		//measures::jain(self.servers.iter().map(|s|s.statistics.current_measurement.created_phits as f64))
		let limit:usize = self.servers.iter().map(|s|s.statistics.temporal_statistics.len()).max().unwrap_or(0);
		let mut temporal_server_defined_statistics_measurement: Vec<Vec<Vec<(Vec<ConfigurationValue>, Vec<f32>, usize)>>> =  vec![vec![vec![]; temporal_server_defined_statistics_definitions.len() ]; limit ];

		for index_cycle in 0..limit
		{
			//(0..limit).map(|index_cycle|{
			for (index,definition) in temporal_server_defined_statistics_definitions.iter().enumerate()
			{
				for server in self.servers.iter()
				{
					let context_content = vec![
						(String::from("generated_phits"), ConfigurationValue::Number(server.statistics.temporal_statistics.get(index_cycle).map(|m|m.created_phits as f64).unwrap_or(0f64))),
						(String::from("accepted_phits"), ConfigurationValue::Number(server.statistics.temporal_statistics.get(index_cycle).map(|m|m.consumed_phits as f64).unwrap_or(0f64))),
						(String::from("missed_generations"), ConfigurationValue::Number(server.statistics.temporal_statistics.get(index_cycle).map(|m|m.missed_generations as f64).unwrap_or(0f64))),
						(String::from("server_index"), ConfigurationValue::Number(server.index as f64)),
						(String::from("switches"), ConfigurationValue::Number( match server.port.0{
							Location::RouterPort {router_index, router_port: _} => router_index as f64,
							_ => panic!("Here there should be a router")
						} )),
					];

					let context = ConfigurationValue::Object( String::from("packet"), context_content );
					let path = Path::new(".");
					let key : Vec<ConfigurationValue> = definition.0.iter().map(|key_expr|config::evaluate( key_expr, &context, path)
						.unwrap_or_else(|error|panic!("error building user defined statistics: {}",error))).collect();

					let value : Vec<f32> = definition.1.iter().map(|key_expr|
						match config::evaluate( key_expr, &context, path).unwrap_or_else(|error|panic!("error building user defined statistics: {}",error)){
							ConfigurationValue::Number(x) => x as f32,
							_ => 0f32,
						}).collect();

					//find the measurement
					let measurement = temporal_server_defined_statistics_measurement[index_cycle][index].iter_mut().find(|m|m.0==key);
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
							temporal_server_defined_statistics_measurement[index_cycle][index].push( (key, value, 1) )
						},
					};
				}
			}
		} //).collect()
		temporal_server_defined_statistics_measurement
		//temporal_packet_defined_statistics_measurement
	}
}

///Description of common properties of sets of links.
///For example, the links to servers could have a different delay.
///The topologies can set additional classes. For example, a mesh/torus can differentiate horizontal/vertical links.
///And a dragonfly topology can differentiate local from global links.
#[derive(Debug,Clone)]
pub struct LinkClass
{
	///Cycles the phit needs to move from one endpoint to the other endpoint.
	pub delay: Time,
	//(x,y) means x phits each y cycles ??
	//transference_speed: (usize,usize)
	///A phit can enter the link only in those cycles multiple of `frequency_divisor`.
	///By default it is set a value of 0, value which will be replaced with the global frequency divisor of the simulation (whose default is 1).
	frequency_divisor: Time,
}

impl LinkClass
{
	fn new(cv:&ConfigurationValue) -> LinkClass
	{
		let mut delay=None;
		let mut frequency_divisor = 0;
		match_object_panic!(cv,"LinkClass",value,
			"delay" => delay=Some(value.as_time().expect("bad value for delay")),
			"frequency_divisor" => frequency_divisor = value.as_time().expect("bad value for frequency_divisor"),
		);
		let delay=delay.expect("There were no delay");
		LinkClass{
			delay,
			frequency_divisor,
		}
	}
}

/**
Part of Simulation that is intended to be exposed to the `Eventful::process` API in a read-only way.
**/
pub struct SimulationShared
{
	///The current cycle, i.e, the current discrete time.
	pub cycle:Time,
	///The instantiated network. It contains the routers and servers connected according to the topology.
	pub network: Network,
	///The traffic being generated/consumed by the servers.
	pub traffic: Box<dyn Traffic>,
	///The routing algorithm that the network router will employ to set candidate routes.
	pub routing: Box<dyn Routing>,
	///The properties associated to each link class.
	pub link_classes: Vec<LinkClass>,
	///The maximum size in phits that network packets can have. Any message greater than this is broken into several packets.
	pub maximum_packet_size: usize,
	/// The base period of operation for the components. Defaults to 1, to allow having events every cycle.
	/// Components using this value will only execute at cycles multiple of it.
	/// This parameter allows to reduce the global frequency, allowing in turn to override some component to have greater frequency than the rest.
	pub general_frequency_divisor: Time,
}

impl SimulationShared
{
	/**
	Whether the current cycle is a multiple of the frequency divisor of the given `link_class`.
	These are the cycles in which it is allowed to send a phit through the link.
	The phit reception event should then be scheduled normally at cycle+delay.
	**/
	pub fn is_link_cycle(&self, link_class: usize) -> bool
	{
		self.cycle % self.link_classes[link_class].frequency_divisor == 0
	}
	/**
		Schedule an event to be executed at the arrival across a link.
		Counts both the wait for the time slot and the delay.
	**/
	pub fn schedule_link_arrival(&self, link_class:usize, event:Event) -> EventGeneration
	{
		let link = &self.link_classes[link_class];
		let slot = event::round_to_multiple(self.cycle,link.frequency_divisor);
		let wait = slot - self.cycle;
		EventGeneration{
			delay: wait + link.delay,
			position: event::CyclePosition::Begin,
			event,
		}
	}
}

/**
Part of Simulation that is intended to be exposed to the `Eventful::process` API in a mutable way.
**/
pub struct SimulationMut
{
	///The random number generator itself, with its current state.
	pub rng: StdRng,
}

///The object representing the whole simulation.
pub struct Simulation<'a>
{
	///The whole parsed configuration.
	#[allow(dead_code)]
	pub configuration: ConfigurationValue,
	///The seed of the random number generator.
	#[allow(dead_code)]
	pub seed: usize,
	///Encapsulated data of the simulation intended to be readable by many.
	pub shared: SimulationShared,
	///Encapsulated data intended to be mutable by any.
	pub mutable: SimulationMut,
	///Cycles of preparation before the actual measured execution
	pub warmup: Time,
	///Cycles of measurement
	pub measured: Time,
	///Maximum number of messages for generation to store in each server. Its default value is 20 messages.
	///Attempts to generate traffic that fails because of the limit are tracked into the `missed_generations` statistic.
	///Note that packets are not generated until it is the turn for the message to be sent to a router.
	pub server_queue_size: usize,
	///The queue of events guiding the simulation.
	pub event_queue: EventQueue,
	///The statistics being collected.
	pub statistics: Statistics,
	///Information abut how to launch simulations to different systems.
	#[allow(dead_code)]
	pub launch_configurations: Vec<ConfigurationValue>,
	///Plugged functions to build traffics, routers, etc.
	pub plugs: &'a Plugs,
	///Number of cycles to wait between reports of memory usage.
	pub memory_report_period: Option<Time>,
}

impl<'a> Simulation<'a>
{
	pub fn new(cv: &ConfigurationValue, plugs:&'a Plugs) -> Simulation<'a>
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
		let mut statistics_message_definitions:Vec< (Vec<Expr>,Vec<Expr>) > = vec![];
		let mut temporal_defined_statistics:Vec< (Vec<Expr>, Vec<Expr>) > = vec![];
		let mut server_queue_size = None;
		let mut memory_report_period = None;
		let mut general_frequency_divisor = 1;
		match_object_panic!(cv,"Configuration",value,
			"random_seed" => seed=Some(value.as_usize().expect("bad value for random_seed")),
			"warmup" => warmup=Some(value.as_time().expect("bad value for warmup")),
			"measured" => measured=Some(value.as_time().expect("bad value for measured")),
			"topology" => topology=Some(value),
			"traffic" => traffic=Some(value),
			"maximum_packet_size" => maximum_packet_size=Some(value.as_usize().expect("bad value for maximum_packet_size")),
			"server_queue_size" => server_queue_size=Some(value.as_usize().expect("bad value for server_queue_size")),
			"router" => router_cfg=Some(value),
			"routing" => routing=Some(new_routing(RoutingBuilderArgument{cv:value,plugs})),
			"link_classes" => link_classes = Some(value.as_array().expect("bad value for link_classes").iter()
				.map(LinkClass::new).collect()),
			"statistics_temporal_step" => statistics_temporal_step=value.as_time().expect("bad value for statistics_temporal_step"),
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
			"statistics_message_definitions" => match value
			{
				&ConfigurationValue::Array(ref l) => statistics_message_definitions=l.iter().map(|definition|match definition {
					&ConfigurationValue::Array(ref dl) => {
						if dl.len()!=2
						{
							panic!("Each definition of statistics_message_definitions must be composed of [keys,values]");
						}
						let keys = match dl[0]
						{
							ConfigurationValue::Array(ref lx) => lx.iter().map(|x|match x{
								ConfigurationValue::Expression(expr) => expr.clone(),
								_ => panic!("bad value for statistics_message_definitions"),
								}).collect(),
							_ => panic!("bad value for statistics_message_definitions"),
						};
						let values = match dl[1]
						{
							ConfigurationValue::Array(ref lx) => lx.iter().map(|x|match x{
								ConfigurationValue::Expression(expr) => expr.clone(),
								_ => panic!("bad value for statistics_message_definitions"),
								}).collect(),
							_ => panic!("bad value for statistics_message_definitions"),
						};
						(keys,values)
					},
					_ => panic!("bad value for statistics_message_definitions"),
				}).collect(),
				_ => panic!("bad value for statistics_message_definitions"),
			}
			"statistics_temporal_definitions" | "temporal_statistics_packet_definitions" => match value
			{
				&ConfigurationValue::Array(ref l) => temporal_defined_statistics=l.iter().map(|definition|match definition {
					&ConfigurationValue::Array(ref dl) => {
						if dl.len()!=2
						{
							panic!("Each definition of temporal_defined_statistics must be composed of [keys,values]");
						}
						let keys = match dl[0]
						{
							ConfigurationValue::Array(ref lx) => lx.iter().map(|x|match x{
								ConfigurationValue::Expression(expr) => expr.clone(),
								_ => panic!("bad value for temporal_defined_statistics"),
								}).collect(),
							_ => panic!("bad value for temporal_defined_statistics"),
						};
						let values = match dl[1]
						{
							ConfigurationValue::Array(ref lx) => lx.iter().map(|x|match x{
								ConfigurationValue::Expression(expr) => expr.clone(),
								_ => panic!("bad value for temporal_defined_statistics"),
								}).collect(),
							_ => panic!("bad value for temporal_defined_statistics"),
						};
						(keys,values)
					},
					_ => panic!("bad value for temporal_defined_statistics"),
				}).collect(),
				_ => panic!("bad value for temporal_defined_statistics"),
			}

			"memory_report_period" => memory_report_period=Some(value.as_time().expect("bad value for memory_report_period")),
			"general_frequency_divisor" => general_frequency_divisor = value.as_time().expect("bad value for general_frequency_divisor"),
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
		let mut link_classes:Vec<LinkClass>=link_classes.expect("There were no link_classes");
		for link_class in link_classes.iter_mut()
		{
			if link_class.frequency_divisor == 0
			{
				link_class.frequency_divisor = general_frequency_divisor;
			}
		}
		//This has been changed from rand-0.4 to rand-0.8
		let mut rng=StdRng::seed_from_u64(seed as u64);
		let topology=new_topology(TopologyBuilderArgument{
			cv:topology,
			plugs,
			rng:&mut rng,
		});
		topology.check_adjacency_consistency(Some(link_classes.len()));
		routing.initialize(topology.as_ref(),&mut rng);
		let num_routers=topology.num_routers();
		let num_servers=topology.num_servers();
		//let routers: Vec<Rc<RefCell<dyn Router>>>=(0..num_routers).map(|index|new_router(index,router_cfg,plugs,topology.as_ref(),maximum_packet_size)).collect();
		let routers: Vec<Rc<RefCell<dyn Router>>>=(0..num_routers).map(|index|new_router(RouterBuilderArgument{
			router_index:index,
			cv:router_cfg,
			plugs,
			topology:topology.as_ref(),
			maximum_packet_size,
			general_frequency_divisor,
			statistics_temporal_step,
			rng:&mut rng,
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
					// let nvc=router.num_virtual_channels();
					// let buffer_amount=nvc;
					// //TODO: this seems that should a function of the TransmissionFromServer...
					// let buffer_size=(0..nvc).map(|vc|router.virtual_port_size(router_port,vc)).max().expect("0 buffers in the router");
					// let size_to_send=maximum_packet_size;
					// let from_server_mechanism = TransmissionFromServer::new(buffer_amount,buffer_size,size_to_send);
					// let status = from_server_mechanism.new_status_at_emissor();
					// Box::new(status)
					router.build_emissor_status(router_port,&*topology)
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
				outcoming_virtual_channel: None,
				consumed_phits: BTreeMap::new(),
				statistics: ServerStatistics::new(statistics_temporal_step),
			}
		}).collect();
		let traffic=new_traffic(TrafficBuilderArgument{
			cv:traffic,
			plugs,
			topology:topology.as_ref(),
			rng:&mut rng,
		});
		let num_tasks = traffic.number_tasks();
		if num_tasks != num_servers
		{
			println!("WARNING: Generating traffic over {} tasks when the topology has {} servers.",num_tasks,num_servers);
		}
		let statistics=Statistics::new(statistics_temporal_step, statistics_server_percentiles, statistics_packet_percentiles, statistics_packet_definitions, statistics_message_definitions, temporal_defined_statistics, topology.as_ref());
		Simulation{
			configuration: cv.clone(),
			seed,
			shared: SimulationShared{
				cycle:0,
				network: Network{
					topology,
					routers,
					servers,
				},
				traffic,
				routing,
				link_classes,
				maximum_packet_size,
				general_frequency_divisor,
			},
			mutable: SimulationMut{
				rng,
			},
			warmup,
			measured,
			server_queue_size,
			event_queue: EventQueue::new(1000),
			statistics,
			launch_configurations,
			plugs,
			memory_report_period,
		}
	}
	///Run the simulations until it finishes.
	pub fn run(&mut self)
	{
		self.print_memory_breakdown();
		self.statistics.print_header();
		while self.shared.cycle < self.warmup+self.measured
		{
			self.advance();
			if self.shared.cycle==self.warmup
			{
				self.statistics.reset(self.shared.cycle,&mut self.shared.network);
				self.shared.routing.reset_statistics(self.shared.cycle);
			}
			if self.shared.traffic.is_finished()
			{
				println!("Traffic consumed before cycle {}",self.shared.cycle);
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
			//if self.shared.cycle>=3122
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
					let target_server = phit.packet.message.destination;
					let (target_location,_link_class)=self.shared.network.topology.server_neighbour(target_server);
					let target_router=match target_location
					{
						Location::RouterPort{router_index,router_port:_} =>router_index,
						_ => panic!("The server is not attached to a router"),
					};
					match new
					{
						&Location::RouterPort{router_index:router,router_port:port} =>
						{
							self.statistics.link_statistics[router][port].phit_arrivals+=1;
							if phit.is_begin() && !self.statistics.packet_defined_statistics_definitions.is_empty()
							{
								let mut be = phit.packet.extra.borrow_mut();
								if be.is_none()
								{
									*be=Some(PacketExtraInfo::default());
								}
								let extra = be.as_mut().unwrap();
								let (_,link_class) = self.shared.network.topology.neighbour(router,port);
								extra.link_classes.push(link_class);
								extra.id_switches.push(router);
								extra.entry_virtual_channels.push(*phit.virtual_channel.borrow());
								extra.cycle_per_hop.push(self.shared.cycle);
							}
							let mut brouter=self.shared.network.routers[router].borrow_mut();
							for event in brouter.insert(self.shared.cycle,phit.clone(),port,&mut self.mutable.rng)
							{
								self.event_queue.enqueue(event);
							}
							match previous
							{
								&Location::ServerPort(_server_index) => if phit.is_begin()
								{
									*phit.packet.cycle_into_network.borrow_mut() = self.shared.cycle;
									self.shared.routing.initialize_routing_info(&phit.packet.routing_info, self.shared.network.topology.as_ref(), router, target_router, Some(target_server), &mut self.mutable.rng);
								},
								&Location::RouterPort{../*router_index,router_port*/} =>
								{
									self.statistics.track_phit_hop(phit,self.shared.cycle);
									if phit.is_begin()
									{
										phit.packet.routing_info.borrow_mut().hops+=1;
										self.shared.routing.update_routing_info(&phit.packet.routing_info, self.shared.network.topology.as_ref(), router, port, target_router, Some(target_server), &mut self.mutable.rng);
									}
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
							self.shared.network.servers[server].consume(phit.clone(),self.shared.traffic.deref_mut(),&mut self.statistics,self.shared.cycle,self.shared.network.topology.as_ref(),&mut self.mutable.rng);
						}
						&Location::None => panic!("Phit went nowhere previous={:?}",previous),
					};
				},
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
						let mut brouter=self.shared.network.routers[router_index].borrow_mut();
						for event in brouter.acknowledge(self.shared.cycle,router_port,ack_message)
						{
							self.event_queue.enqueue(event);
						}
					},
					Location::ServerPort(server) => self.shared.network.servers[server].router_status.acknowledge(ack_message),
					//&Location::ServerPort(server) => TransmissionFromServer::acknowledge(self.shared.network.servers[server].router_status,ack_message),
					_ => (),
				},
				Event::Generic(ref element) =>
				{
					// --- generic events at the START of the cycle ---
					let new_events=element.borrow_mut().process(&self.shared,&mut self.mutable);
					//element.borrow_mut().clear_pending_events();//now done by process itself
					for ge in new_events.into_iter()
					{
						self.event_queue.enqueue(ge);
					}
				},
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
			//if self.shared.cycle>=3122
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
					// --- generic events at the END of the cycle ---
					let new_events=element.borrow_mut().process(&self.shared,&mut self.mutable);
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
		let num_servers=self.shared.network.servers.len();
		for (iserver,server) in self.shared.network.servers.iter_mut().enumerate()
		{
			//println!("credits of {} = {}",iserver,server.credits);
			if let (Location::RouterPort{router_index: index,router_port: port},link_class)=server.port
			{
				if self.shared.traffic.should_generate(iserver,self.shared.cycle,&mut self.mutable.rng)
				{
					if server.stored_messages.len()<self.server_queue_size {
						match self.shared.traffic.generate_message(iserver,self.shared.cycle,self.shared.network.topology.as_ref(),&mut self.mutable.rng)
						{
							Ok(message) =>
							{
								if message.destination>=num_servers
								{
									panic!("Message sent to outside the network unexpectedly. destination={destination}",destination=message.destination);
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
						server.statistics.track_missed_generation(self.shared.cycle);
					}
				}
				if server.stored_packets.is_empty() && !server.stored_messages.is_empty()
				{
					let message=server.stored_messages.pop_front().expect("There are not messages in queue");
					let mut size=message.size;
					let mut index_packet=0;
					while size>0
					{
						let ps=if size>self.shared.maximum_packet_size
						{
							self.shared.maximum_packet_size
						}
						else
						{
							size
						};
						let mut routing_info = RoutingInfo::new();
						routing_info.source_server = Some(iserver);
						server.stored_packets.push_back(Packet{
							size:ps,
							routing_info: RefCell::new(routing_info),
							message:message.clone(),
							index:index_packet,
							cycle_into_network:RefCell::new(0),
							extra: RefCell::new(None),
						}.into_ref());
						index_packet+=1;
						size-=ps;
					}
				}
				if server.stored_phits.is_empty() && !server.stored_packets.is_empty()
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
				if !server.stored_phits.is_empty()
				{
					//Do not extract the phit until we know whether we can transmit it.
					let phit=server.stored_phits.front().expect("There are not phits");
					if let None = server.outcoming_virtual_channel
					{
						// Try to assign one
						assert!(phit.is_begin(),"Not VC assigned for server--router while transmitting a middle phit.");
						let status = &server.router_status;
						for vc in  0..status.num_virtual_channels()
						{
							if status.can_transmit(phit,vc)
							{
								server.outcoming_virtual_channel = Some(vc);
								break;
							}
						}
					}
					// if self.shared.is_link_cycle(link_class) // XXX we cannot call this since we are mutating the servers.
					if self.shared.cycle % self.shared.link_classes[link_class].frequency_divisor == 0
					{
						if let Some(vc) = server.outcoming_virtual_channel
						{
							if server.router_status.can_transmit(phit,vc)
							{
								let phit=server.stored_phits.pop_front().expect("There are not phits");
								*phit.virtual_channel.borrow_mut() = Some(vc);
								if phit.is_end()
								{
									server.outcoming_virtual_channel = None;
								}
								let event=Event::PhitToLocation{
									phit,
									previous: Location::ServerPort(iserver),
									new: Location::RouterPort{router_index:index,router_port:port},
								};
								//self.statistics.created_phits+=1;
								self.statistics.track_created_phit(self.shared.cycle);
								server.statistics.track_created_phit(self.shared.cycle);
								self.event_queue.enqueue_begin(event,self.shared.link_classes[link_class].delay);
								server.router_status.notify_outcoming_phit(vc,self.shared.cycle);
							}
						}
					}
				}
			}
			else
			{
				panic!("Where goes this port?");
			}
		}
		//println!("Done generation");
		self.event_queue.advance();
		self.shared.cycle+=1;
		if self.shared.cycle%1000==0
		{
			//println!("Statistics up to cycle {}: {:?}",self.shared.cycle,self.statistics);
			self.statistics.print(self.shared.cycle,&self.shared.network);
		}
		if let Some(period) = self.memory_report_period
		{
			if self.shared.cycle % period == 0
			{
				self.print_memory_breakdown();
			}
		}
	}
	///Get config value for the simulation results.
	pub fn get_simulation_results(&self) -> ConfigurationValue
	{
		// https://stackoverflow.com/questions/22355273/writing-to-a-file-or-stdout-in-rust
		//output.write(b"Hello from the simulator\n").unwrap();
		//Result
		//{
		//	accepted_load: 0.9,
		//	average_message_delay: 100,
		//}
		let measurement = &self.statistics.current_measurement;
		let cycles=self.shared.cycle-measurement.begin_cycle;
		let num_servers=self.shared.network.servers.len();
		let injected_load=measurement.created_phits as f64/cycles as f64/num_servers as f64;
		let accepted_load=measurement.consumed_phits as f64/cycles as f64/num_servers as f64;
		let average_message_delay=measurement.total_message_delay as f64/measurement.consumed_messages as f64;
		let average_packet_network_delay=measurement.total_packet_network_delay as f64/measurement.consumed_packets as f64;
		let jscp=self.shared.network.jain_server_consumed_phits();
		let jsgp=self.shared.network.jain_server_created_phits();
		let average_packet_hops=measurement.total_packet_hops as f64 / measurement.consumed_packets as f64;
		let total_packet_per_hop_count=measurement.total_packet_per_hop_count.iter().map(|&count|ConfigurationValue::Number(count as f64)).collect();
		//let total_arrivals:usize = self.statistics.link_statistics.iter().map(|rls|rls.iter().map(|ls|ls.phit_arrivals).sum::<usize>()).sum();
		//let total_links:usize = self.statistics.link_statistics.iter().map(|rls|rls.len()).sum();
		let total_arrivals:usize = (0..self.shared.network.topology.num_routers()).map(|i|(0..self.shared.network.topology.degree(i)).map(|j|self.statistics.link_statistics[i][j].phit_arrivals).sum::<usize>()).sum();
		let total_links: usize = (0..self.shared.network.topology.num_routers()).map(|i|self.shared.network.topology.degree(i)).sum();
		let average_link_utilization = total_arrivals as f64 / cycles as f64 / total_links as f64;
		let maximum_arrivals:usize = self.statistics.link_statistics.iter().map(|rls|rls.iter().map(|ls|ls.phit_arrivals).max().unwrap()).max().unwrap();
		let maximum_link_utilization = maximum_arrivals as f64 / cycles as f64;
		let server_average_cycle_last_created_phit : f64 = (self.shared.network.servers.iter().map(|s|s.statistics.cycle_last_created_phit).sum::<Time>() as f64)/(self.shared.network.servers.len() as f64);
		let server_average_cycle_last_consumed_message : f64 = (self.shared.network.servers.iter().map(|s|s.statistics.cycle_last_consumed_message).sum::<Time>() as f64)/(self.shared.network.servers.len() as f64);
		let server_average_missed_generations : f64 = (self.shared.network.servers.iter().map(|s|s.statistics.current_measurement.missed_generations).sum::<usize>() as f64)/(self.shared.network.servers.len() as f64);
		let servers_with_missed_generations : usize = self.shared.network.servers.iter().map(|s|if s.statistics.current_measurement.missed_generations > 0 {1} else {0}).sum::<usize>();
		let virtual_channel_usage: Vec<_> =measurement.virtual_channel_usage.iter().map(|&count|
			ConfigurationValue::Number(count as f64 / cycles as f64 / total_links as f64)
		).collect();
		let git_id=get_git_id();
		let version_number = get_version_number();
		let mut result_content = vec![
			(String::from("cycle"),ConfigurationValue::Number(self.shared.cycle as f64)),
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
			(String::from("virtual_channel_usage"),ConfigurationValue::Array(virtual_channel_usage)),
			//(String::from("git_id"),ConfigurationValue::Literal(format!("\"{}\"",git_id))),
			(String::from("git_id"),ConfigurationValue::Literal(git_id.to_string())),
			(String::from("version_number"),ConfigurationValue::Literal(version_number.to_string())),
		];
		if let Some(content)=self.shared.routing.statistics(self.shared.cycle)
		{
			result_content.push((String::from("routing_statistics"),content));
		}
		if let Some(content) = self.shared.network.routers.iter().enumerate().fold(None,|maybe_stat,(index,router)|router.borrow().aggregate_statistics(maybe_stat,index,self.shared.network.routers.len(),self.shared.cycle))
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
			let mut average_packet_hops_collect = Vec::with_capacity(samples);
			let mut virtual_channel_usage_collect = Vec::with_capacity(samples);
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
				let average_packet_hops=measurement.total_packet_hops as f64 / measurement.consumed_packets as f64;
				average_packet_hops_collect.push(ConfigurationValue::Number(average_packet_hops));
				let virtual_channel_usage: Vec<_> =measurement.virtual_channel_usage.iter().map(|&count|
					ConfigurationValue::Number(count as f64 / step as f64 / total_links as f64)
				).collect();
				virtual_channel_usage_collect.push(ConfigurationValue::Array(virtual_channel_usage));
			};
			let jscp_collect = self.shared.network.temporal_jain_server_consumed_phits()
				.into_iter()
				.map(|x|ConfigurationValue::Number(x))
				.collect();
			let jsgp_collect = self.shared.network.temporal_jain_server_created_phits()
				.into_iter()
				.map(|x|ConfigurationValue::Number(x))
				.collect();
			let temporal_content = vec![
				//(String::from("cycle"),ConfigurationValue::Number(self.shared.cycle as f64)),
				(String::from("injected_load"),ConfigurationValue::Array(injected_load_collect)),
				(String::from("accepted_load"),ConfigurationValue::Array(accepted_load_collect)),
				(String::from("average_message_delay"),ConfigurationValue::Array(average_message_delay_collect)),
				(String::from("average_packet_network_delay"),ConfigurationValue::Array(average_packet_network_delay_collect)),
				(String::from("server_generation_jain_index"),ConfigurationValue::Array(jsgp_collect)),
				(String::from("server_consumption_jain_index"),ConfigurationValue::Array(jscp_collect)),
				(String::from("average_packet_hops"),ConfigurationValue::Array(average_packet_hops_collect)),
				(String::from("virtual_channel_usage"),ConfigurationValue::Array(virtual_channel_usage_collect)),
				//(String::from("total_packet_per_hop_count"),ConfigurationValue::Array(total_packet_per_hop_count)),
				//(String::from("average_link_utilization"),ConfigurationValue::Number(average_link_utilization)),
				//(String::from("maximum_link_utilization"),ConfigurationValue::Number(maximum_link_utilization)),
				//(String::from("git_id"),ConfigurationValue::Literal(format!("{}",git_id))),
			];
			result_content.push((String::from("temporal_statistics"),ConfigurationValue::Object(String::from("TemporalStatistics"),temporal_content)));
		}
		if !self.statistics.server_percentiles.is_empty()
		{
			let mut servers_injected_load : Vec<f64> = self.shared.network.servers.iter().map(|s|s.statistics.current_measurement.created_phits as f64/cycles as f64).collect();
			let mut servers_accepted_load : Vec<f64> = self.shared.network.servers.iter().map(|s|s.statistics.current_measurement.consumed_phits as f64/cycles as f64).collect();
			let mut servers_average_message_delay : Vec<f64> = self.shared.network.servers.iter().map(|s|s.statistics.current_measurement.total_message_delay as f64/s.statistics.current_measurement.consumed_messages as f64).collect();
			let mut servers_cycle_last_created_phit : Vec<Time> = self.shared.network.servers.iter().map(|s|s.statistics.cycle_last_created_phit).collect();
			let mut servers_cycle_last_consumed_message : Vec<Time> = self.shared.network.servers.iter().map(|s|s.statistics.cycle_last_consumed_message).collect();
			let mut servers_missed_generations : Vec<usize> = self.shared.network.servers.iter().map(|s|s.statistics.current_measurement.missed_generations).collect();
			//XXX There are more efficient ways to find percentiles than to sort them, but should not be notable in any case. See https://en.wikipedia.org/wiki/Selection_algorithm
			servers_injected_load.sort_by(|a,b|a.partial_cmp(b).unwrap_or(Ordering::Less));
			servers_accepted_load.sort_by(|a,b|a.partial_cmp(b).unwrap_or(Ordering::Less));
			servers_average_message_delay.sort_by(|a,b|a.partial_cmp(b).unwrap_or(Ordering::Less));
			servers_cycle_last_created_phit.sort_unstable();
			servers_cycle_last_consumed_message.sort_unstable();
			servers_missed_generations.sort_unstable();
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
			let mut packets_delay : Vec<Time> = self.statistics.packet_statistics.iter().map(|ps|ps.delay).collect();
			let num_packets = packets_delay.len();
			if num_packets>0
			{
				let mut packets_hops : Vec<usize> = self.statistics.packet_statistics.iter().map(|ps|ps.hops).collect();
				let mut packets_consumed_cycle: Vec<Time> = self.statistics.packet_statistics.iter().map(|ps|ps.consumed_cycle).collect();
				packets_delay.sort_unstable();
				packets_hops.sort_unstable();
				packets_consumed_cycle.sort_unstable();
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
		if !self.statistics.message_defined_statistics_measurement.is_empty()
		{
			let mut mds_content=vec![];
			for definition_measurement in self.statistics.message_defined_statistics_measurement.iter()
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
					dm_list.push( ConfigurationValue::Object(String::from("MessageBin"),dm_content) );
				}
				mds_content.push(ConfigurationValue::Array(dm_list));
			}
			result_content.push( (String::from("message_defined_statistics"),ConfigurationValue::Array(mds_content)) );
		}
		if !self.statistics.temporal_defined_statistics_measurement.is_empty() && !self.statistics.temporal_defined_statistics_definitions.is_empty()
		{
			let temporal_measurement = self.shared.network.get_temporal_statistics_servers_expr(&self.statistics.temporal_defined_statistics_definitions);
			let mut all_temporal_measurement = vec![];
			for cycle in temporal_measurement
			{
				let mut pds_content=vec![];
				for statistic in cycle
				{
					let mut dm_list = vec![];
					for (key,val,count) in statistic
					{
						let averages = ConfigurationValue::Array( val.iter().map(|v|ConfigurationValue::Number(*v as f64)).collect() );
						let dm_content: Vec<(String,ConfigurationValue)> = vec![
							(String::from("key"),ConfigurationValue::Array(key.to_vec())),
							(String::from("average"),averages),
							(String::from("count"),ConfigurationValue::Number(count as f64)),
						];
						dm_list.push( ConfigurationValue::Object(String::from("PacketBin"),dm_content) );
					}
					pds_content.push(ConfigurationValue::Array(dm_list));
				}
				all_temporal_measurement.push(ConfigurationValue::Array(pds_content));
			}
			result_content.push( (String::from("temporal_defined_statistics"),ConfigurationValue::Array(all_temporal_measurement)) );
		}

		if self.shared.traffic.get_statistics().is_some()
		{
			let traffic_statistics = self.shared.traffic.get_statistics().unwrap().parse_statistics();
			result_content.push((String::from("traffic_statistics"), traffic_statistics));
			// for statistic in traffic_statistics.iter()
			// {
			// 	let s = statistic.borrow();
			//
			//
			// 	let temporal_consumed_messages = s.temporal_statistics.iter().map(|i| ConfigurationValue::Number(i.consumed_messages as f64)).collect::<Vec<_>>();
			// 	let temporal_generated_messages = s.temporal_statistics.iter().map(|i| ConfigurationValue::Number(i.created_messages as f64)).collect::<Vec<_>>();
			// 	let temporal_total_message_delay = s.temporal_statistics.iter().map(|i| ConfigurationValue::Number(
			// 		if i.consumed_messages == 0 {0f64} else{(i.total_message_delay / i.consumed_messages as u64) as f64} )
			// 	).collect::<Vec<_>>();
			//
			// 	let temporal_statistics = vec![
			// 		(String::from("consumed_messages"),ConfigurationValue::Array(temporal_consumed_messages.clone())),
			// 		(String::from("generated_messages"),ConfigurationValue::Array(temporal_generated_messages.clone())),
			// 		(String::from("total_message_delay"),ConfigurationValue::Array(temporal_total_message_delay.clone())),
			// 	];
			//
			// 	let total_consumed_messages = s.total_consumed_messages as f64;
			// 	let total_generated_messages = s.total_created_messages as f64;
			// 	let average_message_delay = s.total_message_delay / total_consumed_messages as u64;
			//
			//
			// 	let traffic_content = vec![
			// 		(String::from("total_consumed_messages"),ConfigurationValue::Number(total_consumed_messages)),
			// 		(String::from("total_generated_messages"),ConfigurationValue::Number(total_generated_messages)),
			// 		(String::from("average_message_delay"),ConfigurationValue::Number(average_message_delay as f64)),
			// 		(String::from("temporal_statistics"), ConfigurationValue::Object(String::from("temporal"),temporal_statistics)),
			// 	];
			// 	traffics_results.push(ConfigurationValue::Object(String::from("TrafficStatistic"),traffic_content));
			// }
		}

		ConfigurationValue::Object(String::from("Result"),result_content)
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
		// let measurement = &self.statistics.current_measurement;
		// let cycles=self.shared.cycle-measurement.begin_cycle;
		// let num_servers=self.shared.network.servers.len();
		// let injected_load=measurement.created_phits as f64/cycles as f64/num_servers as f64;
		// let accepted_load=measurement.consumed_phits as f64/cycles as f64/num_servers as f64;
		// let average_message_delay=measurement.total_message_delay as f64/measurement.consumed_messages as f64;
		// let average_packet_network_delay=measurement.total_packet_network_delay as f64/measurement.consumed_packets as f64;
		// let jscp=self.shared.network.jain_server_consumed_phits();
		// let jsgp=self.shared.network.jain_server_created_phits();
		// let average_packet_hops=measurement.total_packet_hops as f64 / measurement.consumed_packets as f64;
		// let total_packet_per_hop_count=measurement.total_packet_per_hop_count.iter().map(|&count|ConfigurationValue::Number(count as f64)).collect();
		// //let total_arrivals:usize = self.statistics.link_statistics.iter().map(|rls|rls.iter().map(|ls|ls.phit_arrivals).sum::<usize>()).sum();
		// //let total_links:usize = self.statistics.link_statistics.iter().map(|rls|rls.len()).sum();
		// let total_arrivals:usize = (0..self.shared.network.topology.num_routers()).map(|i|(0..self.shared.network.topology.degree(i)).map(|j|self.statistics.link_statistics[i][j].phit_arrivals).sum::<usize>()).sum();
		// let total_links: usize = (0..self.shared.network.topology.num_routers()).map(|i|self.shared.network.topology.degree(i)).sum();
		// let average_link_utilization = total_arrivals as f64 / cycles as f64 / total_links as f64;
		// let maximum_arrivals:usize = self.statistics.link_statistics.iter().map(|rls|rls.iter().map(|ls|ls.phit_arrivals).max().unwrap()).max().unwrap();
		// let maximum_link_utilization = maximum_arrivals as f64 / cycles as f64;
		// let server_average_cycle_last_created_phit : f64 = (self.shared.network.servers.iter().map(|s|s.statistics.cycle_last_created_phit).sum::<Time>() as f64)/(self.shared.network.servers.len() as f64);
		// let server_average_cycle_last_consumed_message : f64 = (self.shared.network.servers.iter().map(|s|s.statistics.cycle_last_consumed_message).sum::<Time>() as f64)/(self.shared.network.servers.len() as f64);
		// let server_average_missed_generations : f64 = (self.shared.network.servers.iter().map(|s|s.statistics.current_measurement.missed_generations).sum::<usize>() as f64)/(self.shared.network.servers.len() as f64);
		// let servers_with_missed_generations : usize = self.shared.network.servers.iter().map(|s|if s.statistics.current_measurement.missed_generations > 0 {1} else {0}).sum::<usize>();
		// let virtual_channel_usage: Vec<_> =measurement.virtual_channel_usage.iter().map(|&count|
		// 	ConfigurationValue::Number(count as f64 / cycles as f64 / total_links as f64)
		// ).collect();
		// let git_id=get_git_id();
		// let version_number = get_version_number();
		// let mut result_content = vec![
		// 	(String::from("cycle"),ConfigurationValue::Number(self.shared.cycle as f64)),
		// 	(String::from("injected_load"),ConfigurationValue::Number(injected_load)),
		// 	(String::from("accepted_load"),ConfigurationValue::Number(accepted_load)),
		// 	(String::from("average_message_delay"),ConfigurationValue::Number(average_message_delay)),
		// 	(String::from("average_packet_network_delay"),ConfigurationValue::Number(average_packet_network_delay)),
		// 	(String::from("server_generation_jain_index"),ConfigurationValue::Number(jsgp)),
		// 	(String::from("server_consumption_jain_index"),ConfigurationValue::Number(jscp)),
		// 	(String::from("average_packet_hops"),ConfigurationValue::Number(average_packet_hops)),
		// 	(String::from("total_packet_per_hop_count"),ConfigurationValue::Array(total_packet_per_hop_count)),
		// 	(String::from("average_link_utilization"),ConfigurationValue::Number(average_link_utilization)),
		// 	(String::from("maximum_link_utilization"),ConfigurationValue::Number(maximum_link_utilization)),
		// 	(String::from("server_average_cycle_last_created_phit"),ConfigurationValue::Number(server_average_cycle_last_created_phit)),
		// 	(String::from("server_average_cycle_last_consumed_message"),ConfigurationValue::Number(server_average_cycle_last_consumed_message)),
		// 	(String::from("server_average_missed_generations"),ConfigurationValue::Number(server_average_missed_generations)),
		// 	(String::from("servers_with_missed_generations"),ConfigurationValue::Number(servers_with_missed_generations as f64)),
		// 	(String::from("virtual_channel_usage"),ConfigurationValue::Array(virtual_channel_usage)),
		// 	//(String::from("git_id"),ConfigurationValue::Literal(format!("\"{}\"",git_id))),
		// 	(String::from("git_id"),ConfigurationValue::Literal(git_id.to_string())),
		// 	(String::from("version_number"),ConfigurationValue::Literal(version_number.to_string())),
		// ];
		// if let Some(content)=self.shared.routing.statistics(self.shared.cycle)
		// {
		// 	result_content.push((String::from("routing_statistics"),content));
		// }
		// if let Some(content) = self.shared.network.routers.iter().enumerate().fold(None,|maybe_stat,(index,router)|router.borrow().aggregate_statistics(maybe_stat,index,self.shared.network.routers.len(),self.shared.cycle))
		// {
		// 	result_content.push((String::from("router_aggregated_statistics"),content));
		// }
		// if let Ok(linux_process) = procfs::process::Process::myself()
		// {
		// 	let status = linux_process.status().expect("failed to get status of the self process");
		// 	if let Some(peak_memory)=status.vmhwm
		// 	{
		// 		//Peak resident set size by kibibytes ("high water mark").
		// 		result_content.push((String::from("linux_high_water_mark"),ConfigurationValue::Number(peak_memory as f64)));
		// 	}
		// 	let stat = linux_process.stat().expect("failed to get stat of the self process");
		// 	let tps = procfs::ticks_per_second().expect("could not get the number of ticks per second.") as f64;
		// 	result_content.push((String::from("user_time"),ConfigurationValue::Number(stat.utime as f64/tps)));
		// 	result_content.push((String::from("system_time"),ConfigurationValue::Number(stat.stime as f64/tps)));
		// }
		// if self.statistics.temporal_step > 0
		// {
		// 	let step = self.statistics.temporal_step;
		// 	let samples = self.statistics.temporal_statistics.len();
		// 	let mut injected_load_collect = Vec::with_capacity(samples);
		// 	let mut accepted_load_collect = Vec::with_capacity(samples);
		// 	let mut average_message_delay_collect = Vec::with_capacity(samples);
		// 	let mut average_packet_network_delay_collect = Vec::with_capacity(samples);
		// 	let mut average_packet_hops_collect = Vec::with_capacity(samples);
		// 	let mut virtual_channel_usage_collect = Vec::with_capacity(samples);
		// 	for measurement in self.statistics.temporal_statistics.iter()
		// 	{
		// 		let injected_load=measurement.created_phits as f64/step as f64/num_servers as f64;
		// 		injected_load_collect.push(ConfigurationValue::Number(injected_load));
		// 		let accepted_load=measurement.consumed_phits as f64/step as f64/num_servers as f64;
		// 		accepted_load_collect.push(ConfigurationValue::Number(accepted_load));
		// 		let average_message_delay=measurement.total_message_delay as f64/measurement.consumed_messages as f64;
		// 		average_message_delay_collect.push(ConfigurationValue::Number(average_message_delay));
		// 		let average_packet_network_delay=measurement.total_message_delay as f64/measurement.consumed_messages as f64;
		// 		average_packet_network_delay_collect.push(ConfigurationValue::Number(average_packet_network_delay));
		// 		let average_packet_hops=measurement.total_packet_hops as f64 / measurement.consumed_packets as f64;
		// 		average_packet_hops_collect.push(ConfigurationValue::Number(average_packet_hops));
		// 		let virtual_channel_usage: Vec<_> =measurement.virtual_channel_usage.iter().map(|&count|
		// 			ConfigurationValue::Number(count as f64 / step as f64 / total_links as f64)
		// 		).collect();
		// 		virtual_channel_usage_collect.push(ConfigurationValue::Array(virtual_channel_usage));
		// 	};
		// 	let jscp_collect = self.shared.network.temporal_jain_server_consumed_phits()
		// 		.into_iter()
		// 		.map(|x|ConfigurationValue::Number(x))
		// 		.collect();
		// 	let jsgp_collect = self.shared.network.temporal_jain_server_created_phits()
		// 		.into_iter()
		// 		.map(|x|ConfigurationValue::Number(x))
		// 		.collect();
		// 	let temporal_content = vec![
		// 		//(String::from("cycle"),ConfigurationValue::Number(self.shared.cycle as f64)),
		// 		(String::from("injected_load"),ConfigurationValue::Array(injected_load_collect)),
		// 		(String::from("accepted_load"),ConfigurationValue::Array(accepted_load_collect)),
		// 		(String::from("average_message_delay"),ConfigurationValue::Array(average_message_delay_collect)),
		// 		(String::from("average_packet_network_delay"),ConfigurationValue::Array(average_packet_network_delay_collect)),
		// 		(String::from("server_generation_jain_index"),ConfigurationValue::Array(jsgp_collect)),
		// 		(String::from("server_consumption_jain_index"),ConfigurationValue::Array(jscp_collect)),
		// 		(String::from("average_packet_hops"),ConfigurationValue::Array(average_packet_hops_collect)),
		// 		(String::from("virtual_channel_usage"),ConfigurationValue::Array(virtual_channel_usage_collect)),
		// 		//(String::from("total_packet_per_hop_count"),ConfigurationValue::Array(total_packet_per_hop_count)),
		// 		//(String::from("average_link_utilization"),ConfigurationValue::Number(average_link_utilization)),
		// 		//(String::from("maximum_link_utilization"),ConfigurationValue::Number(maximum_link_utilization)),
		// 		//(String::from("git_id"),ConfigurationValue::Literal(format!("{}",git_id))),
		// 	];
		// 	result_content.push((String::from("temporal_statistics"),ConfigurationValue::Object(String::from("TemporalStatistics"),temporal_content)));
		// }
		// if !self.statistics.server_percentiles.is_empty()
		// {
		// 	let mut servers_injected_load : Vec<f64> = self.shared.network.servers.iter().map(|s|s.statistics.current_measurement.created_phits as f64/cycles as f64).collect();
		// 	let mut servers_accepted_load : Vec<f64> = self.shared.network.servers.iter().map(|s|s.statistics.current_measurement.consumed_phits as f64/cycles as f64).collect();
		// 	let mut servers_average_message_delay : Vec<f64> = self.shared.network.servers.iter().map(|s|s.statistics.current_measurement.total_message_delay as f64/s.statistics.current_measurement.consumed_messages as f64).collect();
		// 	let mut servers_cycle_last_created_phit : Vec<Time> = self.shared.network.servers.iter().map(|s|s.statistics.cycle_last_created_phit).collect();
		// 	let mut servers_cycle_last_consumed_message : Vec<Time> = self.shared.network.servers.iter().map(|s|s.statistics.cycle_last_consumed_message).collect();
		// 	let mut servers_missed_generations : Vec<usize> = self.shared.network.servers.iter().map(|s|s.statistics.current_measurement.missed_generations).collect();
		// 	//XXX There are more efficient ways to find percentiles than to sort them, but should not be notable in any case. See https://en.wikipedia.org/wiki/Selection_algorithm
		// 	servers_injected_load.sort_by(|a,b|a.partial_cmp(b).unwrap_or(Ordering::Less));
		// 	servers_accepted_load.sort_by(|a,b|a.partial_cmp(b).unwrap_or(Ordering::Less));
		// 	servers_average_message_delay.sort_by(|a,b|a.partial_cmp(b).unwrap_or(Ordering::Less));
		// 	servers_cycle_last_created_phit.sort_unstable();
		// 	servers_cycle_last_consumed_message.sort_unstable();
		// 	servers_missed_generations.sort_unstable();
		// 	for &percentile in self.statistics.server_percentiles.iter()
		// 	{
		// 		let mut index:usize = num_servers * usize::from(percentile) /100;
		// 		if index >= num_servers
		// 		{
		// 			//This happens at least in percentile 100%.
		// 			//We cannot find a value greater than ALL, just return the greatest.
		// 			index = num_servers -1;
		// 		}
		// 		let server_content = vec![
		// 			(String::from("injected_load"),ConfigurationValue::Number(servers_injected_load[index])),
		// 			(String::from("accepted_load"),ConfigurationValue::Number(servers_accepted_load[index])),
		// 			(String::from("average_message_delay"),ConfigurationValue::Number(servers_average_message_delay[index])),
		// 			(String::from("cycle_last_created_phit"),ConfigurationValue::Number(servers_cycle_last_created_phit[index] as f64)),
		// 			(String::from("cycle_last_consumed_message"),ConfigurationValue::Number(servers_cycle_last_consumed_message[index] as f64)),
		// 			(String::from("missed_generations"),ConfigurationValue::Number(servers_missed_generations[index] as f64)),
		// 		];
		// 		result_content.push((format!("server_percentile{}",percentile),ConfigurationValue::Object(String::from("ServerStatistics"),server_content)));
		// 	}
		// }
		// if !self.statistics.packet_percentiles.is_empty()
		// {
		// 	let mut packets_delay : Vec<Time> = self.statistics.packet_statistics.iter().map(|ps|ps.delay).collect();
		// 	let num_packets = packets_delay.len();
		// 	if num_packets>0
		// 	{
		// 		let mut packets_hops : Vec<usize> = self.statistics.packet_statistics.iter().map(|ps|ps.hops).collect();
		// 		let mut packets_consumed_cycle: Vec<Time> = self.statistics.packet_statistics.iter().map(|ps|ps.consumed_cycle).collect();
		// 		packets_delay.sort_unstable();
		// 		packets_hops.sort_unstable();
		// 		packets_consumed_cycle.sort_unstable();
		// 		for &percentile in self.statistics.packet_percentiles.iter()
		// 		{
		// 			let mut index:usize = num_packets * usize::from(percentile) /100;
		// 			if index >= num_packets
		// 			{
		// 				//This happens at least in percentile 100%.
		// 				//We cannot find a value greater than ALL, just return the greatest.
		// 				index = num_packets -1;
		// 			}
		// 			let packet_content = vec![
		// 				(String::from("delay"),ConfigurationValue::Number(packets_delay[index] as f64)),
		// 				(String::from("hops"),ConfigurationValue::Number(packets_hops[index] as f64)),
		// 				(String::from("consumed_cycle"),ConfigurationValue::Number(packets_consumed_cycle[index] as f64)),
		// 			];
		// 			result_content.push((format!("packet_percentile{}",percentile),ConfigurationValue::Object(String::from("PacketStatistics"),packet_content)));
		// 		}
		// 	}
		// }
		// if !self.statistics.packet_defined_statistics_measurement.is_empty()
		// {
		// 	let mut pds_content=vec![];
		// 	for definition_measurement in self.statistics.packet_defined_statistics_measurement.iter()
		// 	{
		// 		let mut dm_list = vec![];
		// 		for (key,val,count) in definition_measurement
		// 		{
		// 			let fcount = *count as f32;
		// 			//One average for each value field
		// 			let averages = ConfigurationValue::Array( val.iter().map(|v|ConfigurationValue::Number(f64::from(v/fcount))).collect() );
		// 			let dm_content: Vec<(String,ConfigurationValue)> = vec![
		// 				(String::from("key"),ConfigurationValue::Array(key.to_vec())),
		// 				(String::from("average"),averages),
		// 				(String::from("count"),ConfigurationValue::Number(*count as f64)),
		// 			];
		// 			dm_list.push( ConfigurationValue::Object(String::from("PacketBin"),dm_content) );
		// 		}
		// 		pds_content.push(ConfigurationValue::Array(dm_list));
		// 	}
		// 	result_content.push( (String::from("packet_defined_statistics"),ConfigurationValue::Array(pds_content)) );
		// }
		// let result=ConfigurationValue::Object(String::from("Result"),result_content);
		let result = self.get_simulation_results();
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
		println!("\nBegin memory report at cycle {}",self.shared.cycle);
		println!("Kernel report:");
		if let Ok(linux_process) = procfs::process::Process::myself()
		{
			let status = linux_process.status().expect("failed to get status of the self process");
			if let Some(peak_memory)=status.vmhwm
			{
				// received in kibibytes
				println!("\tpeak resident size (high water mark): {}",quantify::human_bytes(peak_memory as usize * 1024));
			}
			let stat = linux_process.stat().expect("failed to get stat of the self process");
			let tps = procfs::ticks_per_second().expect("could not get the number of ticks per second.") as f64;
			println!("\tuser time in seconds: {}",stat.utime as f64/tps);
			println!("\tsystem time in seconds: {}",stat.stime as f64/tps);
		}
		println!("Size in bytes of each small structure:");
		println!("\tself : {}",size_of::<Self>());
		//println!("phits on statistics : {}",self.statistics.created_phits-self.statistics.consumed_phits);
		println!("\tphit : {}",size_of::<Phit>());
		println!("\tpacket : {}",size_of::<Packet>());
		println!("\tmessage : {}",size_of::<Message>());
		//println!("topology : {}",size_of::<dyn Topology>());
		//println!("router : {}",size_of::<dyn Router>());
		println!("\tserver : {}",size_of::<Server>());
		println!("\tevent : {}",size_of::<Event>());
		//self.event_queue.print_memory();
		println!("Tracked memory:");
		println!("\tnetwork total : {}",quantify::human_bytes(self.shared.network.total_memory()));
		println!("\ttraffic total : {}",quantify::human_bytes(self.shared.traffic.total_memory()));
		println!("\tevent_queue total : {}",quantify::human_bytes(self.event_queue.total_memory()));
		//println!("\trouting total : {}",quantify::human_bytes(self.shared.routing.total_memory()));
		println!("\tstatistics total : {}",quantify::human_bytes(self.statistics.total_memory()));
		println!("End of memory report\n");
	}
	fn forecast_total_memory(&self) -> usize
	{
		unimplemented!();
	}
}


#[derive(Default)]
pub struct Plugs
{
	routers: BTreeMap<String, fn(RouterBuilderArgument) -> Rc<RefCell<dyn Router>>  >,
	topologies: BTreeMap<String, fn(TopologyBuilderArgument) -> Box<dyn Topology> >,
	stages: BTreeMap<String, fn(StageBuilderArgument) -> Box<dyn Stage> >,
	routings: BTreeMap<String,fn(RoutingBuilderArgument) -> Box<dyn Routing>>,
	traffics: BTreeMap<String,fn(TrafficBuilderArgument) -> Box<dyn Traffic> >,
	patterns: BTreeMap<String, fn(PatternBuilderArgument) -> Box<dyn Pattern> >,
	policies: BTreeMap<String, fn(VCPolicyBuilderArgument) -> Box<dyn VirtualChannelPolicy> >,
	allocators: BTreeMap<String, fn(AllocatorBuilderArgument) -> Box<dyn Allocator> >,
}

impl Plugs
{
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
	fn fmt(&self,f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error>
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
/// `plugs` contains the plugged builder functions.
/// `result_file` indicates where to write the results.
/// `free_args` are free arguments. Those of the form `path=value` are used to override configurations.
pub fn file_main(file:&mut File, plugs:&Plugs, mut results_file:Option<File>,free_args:&[String]) -> Result<(),Error>
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
							println!("experiment {} of {} is {}",i,experiments.len(),experiment.format_terminal());
							let mut simulation=Simulation::new(experiment,plugs);
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
	Ok(())
}


/// Main when passed a directory as path
/// `path` must be a directory containing a `main.cfg`.
/// `plugs` contains the plugged builder functions.
/// `action` is the action to be performed in the experiment. For example running the simulations or drawing graphics.
/// `options` encapsulate other parameters such as restricting the performed action to a range of simulations.
//pub fn directory_main(path:&Path, binary:&str, plugs:&Plugs, option_matches:&Matches)
pub fn directory_main(path:&Path, binary:&str, plugs:&Plugs, action:Action, options: ExperimentOptions) -> Result<(),Error>
{
	if !path.exists()
	{
		println!("Folder {:?} does not exists; creating it.",path);
		fs::create_dir(&path).expect("Something went wrong when creating the main path.");
	}
	let binary_path=Path::new(binary);
	//let mut experiment=Experiment::new(binary_path,path,plugs,option_matches);
	let mut experiment=Experiment::new(binary_path,path,plugs,options);
	experiment.execute_action(action).map_err(|error|error.with_message(format!("Execution of the action {action} failed.")))
	//match experiment.execute_action(action)
	//{
	//	Ok(()) => (),
	//	Err(error) =>
	//	{
	//		eprintln!("Execution the action {} failed with errors:\n{}",action,error);
	//	}
	//}
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


/// The default options to be used in a terminal application.
/// You could build the `ExperimentOptions` directly instead.
pub fn terminal_default_options() -> getopts::Options
{
	let mut opts = getopts::Options::new();
	opts.optopt("a","action","selected action to execute (for directory experiment)","METHOD");
	opts.optopt("r","results","file in which to write the simulation results (for file experiment)","FILE");
	opts.optopt("s","start_index","experiment index in which to start processing","INDEX");
	opts.optopt("e","end_index","experiment index in which to end processing","INDEX");
	opts.optopt("x","special","some special execution","SPECIAL_VALUE");
	opts.optopt("","special_args","arguments for special execution","SPECIAL_VALUE");
	opts.optopt("f","source","copy matching results from another path experiment","PATH");
	opts.optopt("w","where","select the subset of indices for which the configuration expression evaluates to true","EXPRESION");
	opts.optopt("m","message","write a message into the journal file","TEXT");
	opts.optopt("i","interactive","whether to ask for confirmation","BOOLEAN");
	opts.optopt("","use_csv","Use a CSV file as a source for the generations of outputs.","FILE");
	opts.optopt("t","target","Select a target to generate. And skip the rest.","NAME");
	opts.optflag("h","help","show this help");
	opts.optflag("","foreign","Assume to be working with foreign data. Many checks are relaxed.");
	opts
}

/// The final part of the standard main. For when the command line arguments have been processed and no special case is to be run.
pub fn terminal_main_normal_opts(args:&[String], plugs:&Plugs, option_matches:getopts::Matches) -> Result<(),Error>
{
	let action=if option_matches.opt_present("action")
	{
		use std::str::FromStr;
		Action::from_str(&option_matches.opt_str("action").unwrap())?
		//match Action::from_str(&option_matches.opt_str("action").unwrap())
		//{
		//	Ok(action) => action,
		//	Err(e) =>
		//	{
		//		eprintln!("Could not parse the action.\n{e:?}");
		//		std::process::exit(-1);
		//	}
		//}
	}
	else
	{
		Action::LocalAndOutput
	};
	let path=Path::new(&option_matches.free[0]);
	if path.is_dir() || (!path.exists() && match action {Action::Shell=>true,_=>false} )
	{
		if option_matches.free.len()>1
		{
			println!("WARNING: there are {} excess free arguments. This first fre argument is the path the rest is ignored.",option_matches.free.len());
			println!("non-ignored arg {} is {}",0,option_matches.free[0]);
			for (i,free_arg) in option_matches.free.iter().enumerate().skip(1)
			{
				println!("ignored arg {} is {}",i,free_arg);
			}
		}

		let mut options= ExperimentOptions::default();
		if option_matches.opt_present("source")
		{
			options.external_source = Some(Path::new(&option_matches.opt_str("source").unwrap()).to_path_buf());
		}
		if option_matches.opt_present("start_index")
		{
			options.start_index = Some(option_matches.opt_str("start_index").unwrap().parse::<usize>().expect("non-usize received from --start_index"));
		}
		if option_matches.opt_present("end_index")
		{
			options.end_index = Some(option_matches.opt_str("end_index").unwrap().parse::<usize>().expect("non-usize received from --end_index"));
		}
		if option_matches.opt_present("where")
		{
			let expr = match config_parser::parse_expression(&option_matches.opt_str("where").unwrap()).expect("error parsing the where clause")
			{
				config_parser::Token::Expression(expr) => expr,
				x =>
				{
					eprintln!("The where clause is not an expression ({:?}), which it should be.",x);
					std::process::exit(-1);
				}
			};
			options.where_clause = Some(expr);
		}
		if option_matches.opt_present("message")
		{
			options.message = Some(option_matches.opt_str("message").unwrap());
		}
		if option_matches.opt_present("interactive")
		{
			let s = option_matches.opt_str("interactive").unwrap();
			options.interactive = match s.as_ref()
			{
				"" | "true" | "yes" | "y" => Some(true),
				"false" | "no" | "n" => Some(false),
				"none" => None,
				_ =>
				{
					eprintln!("--interactive={s} is not a valid option.");
					std::process::exit(-1);
				}
			};
		}
		if option_matches.opt_present("target")
		{
			let s = option_matches.opt_str("target").unwrap();
			options.targets=Some(vec![s]);
		}
		if option_matches.opt_present("foreign")
		{
			options.foreign=true;
		}
		if option_matches.opt_present("use_csv")
		{
			options.use_csv = Some(Path::new(&option_matches.opt_str("use_csv").unwrap()).to_path_buf());
		}
		return directory_main(&path,&args[0],&plugs,action,options);
	}
	else
	{
		let mut f = File::open(&path).map_err(|err|error!(could_not_open_file,path.to_path_buf(),err).with_message("could not open configuration file.".to_string()))?;
		let results_file= if option_matches.opt_present("results")
		{
			Some(File::create(option_matches.opt_str("results").unwrap()).expect("Could not create results file"))
		}
		else
		{
			None
		};
		//let free_args = option_matches.free.iter().skip(1).collect();
		let free_args=&option_matches.free[1..];
		return file_main(&mut f,&plugs,results_file,free_args);
	}
}

pub fn special_export(args: &str, plugs:&Plugs)
{
	let topology_cfg = match config_parser::parse(args)
	{
		Ok(x) => match x
		{
			config_parser::Token::Value(value) => value,
			_ => panic!("Not a value"),
		},
		Err(x) => panic!("Error parsing topology to export ({:?})",x),
	};
	let mut topology = None;
	let mut seed = None;
	let mut format = None;
	let mut filename = None;
	if let ConfigurationValue::Object(ref cv_name, ref cv_pairs)=topology_cfg
	{
		if cv_name!="Export"
		{
			panic!("A Export must be created from a `Export` object not `{}`",cv_name);
		}
		for &(ref name,ref value) in cv_pairs
		{
			//match name.as_ref()
			match AsRef::<str>::as_ref(&name)
			{
				"topology" =>
				{
					topology=Some(value);
				},
				"seed" => match value
				{
					&ConfigurationValue::Number(f) => seed=Some(f as usize),
					_ => panic!("bad value for seed"),
				},
				"format" => match value
				{
					&ConfigurationValue::Number(f) => format=Some(f as usize),
					_ => panic!("bad value for format"),
				},
				"filename" => match value
				{
					&ConfigurationValue::Literal(ref s) => filename=Some(s.to_string()),
					_ => panic!("bad value for filename"),
				},
				_ => panic!("Nothing to do with field {} in Export",name),
			}
		}
	}
	else
	{
		panic!("Trying to create a Export from a non-Object");
	}
	let seed=seed.unwrap_or(42);
	let topology_cfg=topology.expect("There were no topology.");
	let format=format.unwrap_or(0);
	let filename=filename.expect("There were no filename.");
	let mut rng=StdRng::from_seed({
		//changed from rand-0.4 to rand-0.8
		let mut std_rng_seed = [0u8;32];
		for (index,value) in seed.to_ne_bytes().iter().enumerate()
		{
			std_rng_seed[index]=*value;
		}
		std_rng_seed
	});
	let topology = new_topology(TopologyBuilderArgument{cv:&topology_cfg,plugs,rng:&mut rng});
	let mut topology_file=File::create(&filename).expect("Could not create topology file");
	topology.write_adjacencies_to_file(&mut topology_file,format).expect("Failed writing topology to file");
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
