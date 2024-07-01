
/*!

A [Traffic] defines the way their tasks generate load. In the root traffic of a simulation there should be as many tasks as servers. Traffics with other number of tasks can be combined into a such main traffic.

see [`new_traffic`](fn.new_traffic.html) for documentation on the configuration syntax of predefined traffics.

*/

mod collectives;
mod sequences;
mod mini_apps;
mod basic;
mod operations;

use crate::traffic::mini_apps::{MiniApp, TrafficCredit};
use crate::traffic::collectives::MessageBarrier;
use crate::traffic::collectives::MPICollective;
use crate::traffic::sequences::MessageTaskSequence;
use crate::traffic::sequences::Sequence;
use crate::traffic::sequences::TimeSequenced;
use crate::traffic::sequences::MultimodalBurst;
use std::boxed::Box;
use std::rc::Rc;
use std::fmt::Debug;

use ::rand::{rngs::StdRng};

use crate::config_parser::ConfigurationValue;
use crate::{Message,Plugs};
use crate::topology::Topology;
use crate::event::Time;
use crate::measures::TrafficStatistics;
use crate::quantify::Quantifiable;
use crate::traffic::basic::{Burst, Homogeneous, PeriodicBurst, Reactive, Sleep, SubRangeTraffic, TrafficMessages};
use crate::traffic::operations::{BoundedDifference, ProductTraffic, Shifted, Sum, TrafficMap};

///Possible errors when trying to generate a message with a `Traffic`.
#[derive(Debug)]
pub enum TrafficError
{
	///The traffic tried to send a message outside the network range.
	OriginOutsideTraffic,
	///A task has generated a message to itself. Not necessarily an error.
	SelfMessage,
}

#[derive(Debug)]
pub enum TaskTrafficState
{
	///The task is currently generating traffic.
	Generating,
	///The task is currently waiting to receive some message from others.
	///If the task is known to not going to generate any more traffic it should be a `FinishedGenerating` state instead.
	WaitingData,
	///The task is not going to generate traffic nor change state until the `cycle`.
	WaitingCycle{cycle:Time},
	///The task is not generating traffic for some other reasons.
	UnspecifiedWait,
	///This task will not generate more traffic, but perhaps it will consume.
	FinishedGenerating,
	///This task has nothing else to do within this `Traffic`.
	Finished,
}

///A traffic to be offered to a network. Each task may generate and consume messages.
///Each should call `should_generate` every cycle unless it is unable to store more messages.
pub trait Traffic : Quantifiable + Debug
{
	///Returns a new message following the indications of the traffic.
	fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>;
	///Get its probability of generating per cycle
	fn probability_per_cycle(&self, task:usize) -> f32;
	///If the message was generated by the traffic updates itself and returns true
	///The argument `task` is the one consuming the message.
	//fn try_consume(&mut self, task:usize, message: Rc<Message>, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool;
	fn try_consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool;
	///Indicates if the traffic is not going to generate any more messages.
	///Should be true if and only if the state of all tasks is `Finished`.
	fn is_finished(&self) -> bool;
	///Returns true if a task should generate a message this cycle
	///Should coincide with having the `Generating` state for deterministic traffics.
	fn should_generate(&mut self, _task:usize, _cycle:Time, _rng: &mut StdRng) -> bool
	{
		panic!("should_generate not implemented for this traffic. The default implementation has been removed.");
		// let p=self.probability_per_cycle(task);
		// let r=rng.gen_range(0f32..1f32);
		// r<p
	}
	///Indicates the state of the task within the traffic.
	fn task_state(&self, task:usize, cycle:Time) -> TaskTrafficState;

	/// Indicates the number of tasks in the traffic.
	/// A task is a process that generates traffic.
	fn number_tasks(&self) -> usize;

	fn get_statistics(&self) -> Option<TrafficStatistics> {
		None
	}
}

#[derive(Debug)]
pub struct TrafficBuilderArgument<'a>
{
	///A ConfigurationValue::Object defining the traffic.
	pub cv: &'a ConfigurationValue,
	///The user defined plugs. In case the traffic needs to create elements.
	pub plugs: &'a Plugs,
	///The topology of the network that is gonna to receive the traffic.
	pub topology: &'a dyn Topology,
	///The random number generator to use.
	pub rng: &'a mut StdRng,
}

/**Build a new traffic.

## Base traffics.

### Homogeneous
[Homogeneous] is a traffic where all tasks behave equally and uniform in time. Some `pattern` is generated
by `tasks` number of involved tasks along the whole simulation. Each task tries to use its link toward the network a `load`
fraction of the cycles. The generated messages has a size in phits of `message_size`. The generation is the typical Bernoulli process.

Example configuration.
```ignore
HomogeneousTraffic{
	pattern:Uniform,
	tasks:1000,
	load: 0.9,
	message_size: 16,
}
```

### Burst
In the [Burst] traffic each of the involved `tasks` has a initial list of `messages_per_task` messages to emit. When all the messages
are consumed the simulation is requested to end.
```ignore
Burst{
	pattern:Uniform,
	tasks:1000,
	messages_per_task:200,
	message_size: 16,
}
```

### Reactive

A [Reactive] traffic is composed of an `action_traffic` generated normally, whose packets, when consumed create a response by the `reaction_traffic`.
If both subtraffics are requesting to end and there is no pending message the reactive traffic also requests to end.
```ignore
Reactive{
	action_traffic:HomogeneousTraffic{...},
	reaction_traffic:HomogeneousTraffic{...},
}
```

## Operations

### TrafficSum

[TrafficSum](Sum) generates several traffic at once. Each task generates load for all the traffics, if the total load allows it.
```ignore
TrafficSum{
	list: [HomogeneousTraffic{...},... ],
}
```

### ShiftedTraffic

A [ShiftedTraffic](Shifted) shifts a given traffic a certain amount of tasks. Yu should really check if some pattern transformation fit your purpose, since it will be simpler.
```ignore
ShiftedTraffic{
	traffic: HomogeneousTraffic{...},
	shift: 50,
}
```

### ProductTraffic

A [ProductTraffic] divides the tasks into blocks. Each group generates traffic following the `block_traffic`, but instead of having the destination in the same block it is selected a destination by using the `global_pattern` of the block. Blocks of interest are
* The tasks attached to a router. Then if the global_pattern is a permutation, all the tasks will comunicate with tasks attached to the same router. This can stress the network a lot more than a permutation of tasks.
* All tasks in a group of a dragonfly. If the global_pattern is a permutation, there is only a global link between groups, and Shortest routing is used, then all the packets generated in a group will try by the same global link. Other global links being unused.
Note there is also a product at pattern level, which may be easier to use.

```ignore
ProductTraffic{
	block_size: 10,
	block_traffic: HomogeneousTraffic{...},
	global_pattern: RandomPermutation,
}
```

### SubRangeTraffic

A [SubRangeTraffic] makes tasks outside the range to not generate traffic.
```ignore
SubRangeTraffic{
	start: 100,
	end: 200,
	traffic: HomogeneousTraffic{...},
}
```

### TimeSequenced

[TimeSequenced] defines a sequence of traffics with the given finalization times.

```ignore
TimeSequenced{
	traffics: [HomogeneousTraffic{...}, HomogeneousTraffic{...}],
	times: [2000, 15000],
}
```

### Sequence

Defines a [Sequence] of traffics. When one is completed the next starts.

```ignore
Sequence{
	traffics: [Burst{...}, Burst{...}],
}
```

## Meta traffics

### TrafficMap

A [TrafficMap] applies a map over the tasks of a traffic. This can be used to shuffle the tasks in a application, as in the following example.

```ignore
TrafficMap{
	tasks: 1000,
	application: HomogeneousTraffic{...},
	map: RandomPermutation,
}
```

A [TrafficMap] also can map the set of tasks into a greater set. This is, a small application can be seen as a large one in which many tasks do nothing. This is useful to combine several traffics into one. See its documentation for more details.

*/
pub fn new_traffic(arg:TrafficBuilderArgument) -> Box<dyn Traffic>
{
	if let &ConfigurationValue::Object(ref cv_name, ref _cv_pairs)=arg.cv
	{
		if let Some(builder) = arg.plugs.traffics.get(cv_name)
		{
			return builder(arg);
		}
		match cv_name.as_ref()
		{
			"HomogeneousTraffic" => Box::new(Homogeneous::new(arg)),
			"TrafficSum" => Box::new(Sum::new(arg)),
			"ShiftedTraffic" => Box::new(Shifted::new(arg)),
			"ProductTraffic" => Box::new(ProductTraffic::new(arg)),
			"SubRangeTraffic" => Box::new(SubRangeTraffic::new(arg)),
			"Burst" => Box::new(Burst::new(arg)),
			"MultimodalBurst" => Box::new(MultimodalBurst::new(arg)),
			"Reactive" => Box::new(Reactive::new(arg)),
			"TimeSequenced" => Box::new(TimeSequenced::new(arg)),
			"Sequence" => Box::new(Sequence::new(arg)),
			"BoundedDifference" => Box::new(BoundedDifference::new(arg)),
			"TrafficMap" => Box::new(TrafficMap::new(arg)),
			"PeriodicBurst" => Box::new(PeriodicBurst::new(arg)),
			"Sleep" => Box::new(Sleep::new(arg)),
			"TrafficCredit" => Box::new(TrafficCredit::new(arg)),
			"Messages" => Box::new(TrafficMessages::new(arg)),
			"MessageTaskSequence" => Box::new(MessageTaskSequence::new(arg)),
			"MessageBarrier" => Box::new(MessageBarrier::new(arg)),
			"AllReduce" | "ScatterReduce" | "AllGather" | "All2All" => MPICollective::new(cv_name.clone(), arg),
			"Wavefront" => MiniApp::new(cv_name.clone(), arg),
			_ => panic!("Unknown traffic {}",cv_name),
		}
	}
	else
	{
		panic!("Trying to create a traffic from a non-Object");
	}
}