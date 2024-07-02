use crate::packet::ReferredPayload;
use crate::AsMessage;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use rand::Rng;
use crate::{match_object_panic, Message, Time};
use crate::measures::TrafficStatistics;
use crate::pattern::{new_pattern, Pattern, PatternBuilderArgument};
use crate::topology::Topology;
use crate::traffic::{new_traffic, TaskTrafficState, Traffic, TrafficBuilderArgument, TrafficError};
use crate::traffic::TaskTrafficState::{Finished, Generating, WaitingData};
use crate::ConfigurationValue;


/**
Applies a map over the tasks of a traffic. The source and destination sets may differ. A simple example is to shuffle the tasks in a application, as in the following configuration.

```ignore
TrafficMap{
	tasks: 1000,
	application: HomogeneousTraffic{...},
	map: RandomPermutation,
}
```

TrafficMap also gives the possibility of seeing a small application as a large, helping in the composition of large applications.
The following example uses TrafficMap together with [TrafficSum](Sum),
[CartesianEmbedding](crate::pattern::CartesianEmbedding), [Composition](crate::pattern::Composition),
and [CartesianTransform](crate::pattern::CartesianTransform) to divide the network into
two regions, each employing a different kind of traffic.
```ignore
TrafficSum
{
	list: [
		TrafficMap{
			tasks: 150,
			map: CartesianEmbedding{
				source_sides: [3,5,5],
				destination_sides: [3,10,5],
			},
			application: HomogeneousTraffic{
				pattern: Uniform,
				tasks: 75,
				load: 1.0,
				message_size: 16,
			},
		},
		TrafficMap{
			tasks: 150,
			map: Composition{patterns:[
				CartesianEmbedding{
					source_sides: [3,5,5],
					destination_sides: [3,10,5],
				},
				CartesianTransform{
					sides: [3,10,5],
					shift: [0,5,0],
				},
			]},
			application: Burst{
				pattern: Uniform,
				tasks: 75,
				message_size: 16,
				messages_per_task: 100,
			},
		},
	],
},
```

The `map` is computed once when the traffic is created. Thus, it is recommended for the [Pattern] indicated by `map` to be idempotent.

Currently, the `map` is required to be injective. This is, two tasks must not be mapped into a single one. This restriction could be lifted in the future.
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct TrafficMap
{
    // src_machine -> src_app -> dst_app -> dst_machine

    /// Maps the origin of the traffic.
    /// (source_machine -> source_app)
    from_machine_to_app: Vec<Option<usize>>,

    /// Maps the destination of the traffic.
    /// (destination_app -> destination_machine)
    from_app_to_machine: Vec<usize>,

    /// The traffic to be mapped.
    /// (source_app -> destination_app)
    application: Box<dyn Traffic>,

    /// The number of tasks in the traffic.
    number_tasks: usize,

    /// The map to be applied to the traffic.
    map: Box<dyn Pattern>,

    ///Set of generated messages.
    generated_messages: BTreeMap<*const Message,Rc<Message>>,
}

impl Traffic for TrafficMap
{
    fn generate_message(&mut self, origin: usize, cycle: Time, topology: &dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>, TrafficError>
    {
        // the machine origin of the message
        if origin >= self.from_machine_to_app.len()
        {
            return Err(TrafficError::OriginOutsideTraffic);
        }

        // Get the origin of the message (the app) from the base map
        let app_origin = self.from_machine_to_app[origin].expect("There was no origin for the message");

        // generate the message from the application
        let app_message = self.application.generate_message(app_origin, cycle, topology, rng)?;

        let app_destination = app_message.destination;

        // build the message
        let message = Rc::new(Message{
            origin,
            destination: self.from_app_to_machine[app_destination], // get the destination of the message (the machine) from the base map
            size: app_message.size,
            creation_cycle: app_message.creation_cycle,
            cycle_into_network: RefCell::new(None),
        });
        self.generated_messages.insert(message.as_ref() as *const Message, app_message);
        Ok(message)
    }

    fn probability_per_cycle(&self, task: usize) -> f32
    {
        // The probability of a task is the same as the probability of the task in the application
        let task_app = self.from_machine_to_app[task];

        task_app.map(|app| {
            // get the probability of the task in the application
            self.application.probability_per_cycle(app)
        }).unwrap_or(0.0) // if the task_app has no origin, it has no probability
    }

    fn try_consume(&mut self, task: usize, message: &dyn AsMessage, cycle: Time, topology: &dyn Topology, rng: &mut StdRng) -> bool
    {
        // TODO: Maybe we want to return a Result instead of a bool

        let cycle_into_network = *message.cycle_into_network.borrow();
        let message_ptr = message.as_ref() as *const Message;
        let app_message = match self.generated_messages.remove(&message_ptr)
        {
            Some(app_message) => app_message,
            None => return false,
        };

        let task_app = self.from_machine_to_app[task].expect("There was no origin for the message");

        app_message.cycle_into_network.replace(cycle_into_network);

        // try to consume the message in the application
        self.application.try_consume(task_app, app_message, cycle, topology, rng)
    }


    fn is_finished(&self) -> bool
    {
        self.application.is_finished()
    }

    fn should_generate(&mut self, task: usize, cycle: Time, rng: &mut StdRng) -> bool {
        let task_app = self.from_machine_to_app[task];

        task_app.map(|app| {
            self.application.should_generate(app, cycle, rng)
        }).unwrap_or(false)
    }

    fn task_state(&self, task: usize, cycle: Time) -> Option<TaskTrafficState>
    {
        let task_app = self.from_machine_to_app[task];
        if let Some(app) = task_app
        {
            self.application.task_state(app, cycle).into()
        }
        else
        {
            None
        }

    }

    fn number_tasks(&self) -> usize {
        self.number_tasks
    }

    fn get_statistics(&self) -> Option<TrafficStatistics> {
        self.application.get_statistics()
    }
}


impl TrafficMap
{
    pub fn new(mut arg:TrafficBuilderArgument) -> TrafficMap
    {
        let mut application = None;
        let mut map = None;
        let mut number_tasks = None;
        match_object_panic!(arg.cv,"TrafficMap",value,
			"tasks" => number_tasks=Some(value.as_f64().expect("bad value for tasks") as usize),
			"application" => application = Some(new_traffic(TrafficBuilderArgument{cv:value,rng:&mut arg.rng,..arg})), //traffic of the application
			"map" => map = Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})), //map of the application over the machine
		);

        let number_tasks = number_tasks.expect("There were no tasks in configuration of TrafficMap.");
        let application = application.expect("There were no application in configuration of TrafficMap.");
        let mut map = map.expect("There were no map in configuration of TrafficMap.");

        let app_tasks = application.number_tasks();
        map.initialize(app_tasks, number_tasks, arg.topology, arg.rng);

        let from_app_to_machine: Vec<_> = (0..app_tasks).map(|inner_origin| {
            map.get_destination(inner_origin, arg.topology, arg.rng)
        }).collect();

        // from_machine_to_app is the inverse of from_app_to_machine
        let mut from_machine_to_app = vec![None; number_tasks];
        for i in 0..app_tasks
        {
            from_machine_to_app[from_app_to_machine[i]] = Some(i);
        }

        TrafficMap
        {
            application,
            from_machine_to_app,
            from_app_to_machine,
            number_tasks,
            map,
            generated_messages: BTreeMap::new(),
        }
    }
}


/**
Traffic which is the sum of a list of other traffics.
While it will clearly work when the sum of the generation rates is at most 1, it should behave nicely enough otherwise.

All the subtraffics in `list` must give the same value for `number_tasks`, which is also used for TrafficSum. At least one such subtraffic must be provided.

```ignore
TrafficSum{
	list: [HomogeneousTraffic{...},... ],
	statistics_temporal_step: 1000, //step to record temporal statistics for each subtraffic.
	box_size: 1000, //group results for the messages histogram.
	finish_when: [0, 1] //finish when the first and second subtraffics are finished.
}
```

TODO: document new arguments.
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Sum
{
    ///List of traffic summands
    list: Vec<Box<dyn Traffic>>,
    ///For each task, the index of the traffic that is generating messages.
    index_to_generate: Vec<Vec<usize>>,
    ///Statistics for the traffic
    statistics: TrafficStatistics,
    ///Total number of tasks
    tasks:usize,
    ///Indicate the traffics that should be finished to finish the traffic.
    finish_when: Vec<usize>,
}

impl Traffic for Sum
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {

        // let mut traffics: Vec<&mut Box<dyn Traffic>> = self.list.iter_mut().filter(|t|t.should_generate(origin, cycle, rng)).collect();
        // let probs:Vec<f32>  = traffics.iter().map(|t|t.probability_per_cycle(origin)).collect();
        //
        // //let mut r=rng.gen_range(0f32,probs.iter().sum());//rand-0.4
        // if traffics.len() == 0{
        // 	panic!("This origin is not generating messages in any Traffic")
        // }
        // if traffics.len() > 1{
        // 	panic!("Warning: Multiple traffics are generating messages in the same task.");
        // }
        if self.index_to_generate[origin].len() == 0{
            panic!("This origin is not generating messages in any Traffic")
        }
        if self.index_to_generate[origin].len() > 1 && self.index_to_generate[origin].iter().min().unwrap() != self.index_to_generate[origin].iter().max().unwrap(){
            panic!("Warning: Multiple traffics are generating messages in the same task.");
        }

        let r=rng.gen_range(0..self.index_to_generate[origin].len());//rand-0.8
        let index = self.index_to_generate[origin][r];
        let message = self.list[index].generate_message(origin,cycle,topology,rng);

        if !message.is_err(){
            let size_msg = message.as_ref().unwrap().size;
            self.statistics.track_created_message(cycle, size_msg, Some( index ));
        }
        message

        // for i in 0..traffics.len()
        // {
        // 	if r<probs[i]
        // 	{
        // 		let message = traffics[i].generate_message(origin,cycle,topology,rng);
        // 		if !message.is_err(){
        // 			self.statistics[i].borrow_mut().track_created_message(cycle);
        // 		}
        // 		return message;
        // 	}
        // 	else
        // 	{
        // 		r-=probs[i];
        // 	}
        // }
        // panic!("failed probability");
    }
    //fn should_generate(&self, rng: &mut StdRng) -> bool
    //{
    //	let r=rng.gen_range(0f32,1f32);
    //	r<=self.list.iter().map(|t|t.probability_per_cycle()).sum()
    //}
    fn probability_per_cycle(&self,task:usize) -> f32
    {
        self.list.iter().map(|t|t.probability_per_cycle(task)).sum()
    }
    fn try_consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        for (index, traffic) in self.list.iter_mut().enumerate()
        {
            if traffic.try_consume(task,message.clone(),cycle,topology,rng)
            {
                let injection_time = message.cycle_into_network.borrow().unwrap();
                if injection_time < message.creation_cycle
                {
                    println!("The message was created at cycle {}, injected at cycle {}, and consumed at cycle {}",message.creation_cycle, injection_time, cycle);
                    panic!("Message was injected before it was created")
                }
                self.statistics.track_consumed_message(cycle, cycle - message.creation_cycle, injection_time - message.creation_cycle, message.size, Some(index) );
                return true; //IF SELF MESSAGE ???
            }
        }
        return false;
    }
    fn is_finished(&self) -> bool
    {
        for traffic in self.finish_when.iter()
        {
            if !self.list[*traffic].is_finished()
            {
                return false;
            }
        }
        return true;
    }
    fn should_generate(&mut self, task:usize, cycle:Time, rng: &mut StdRng) -> bool
    {
        self.index_to_generate[task].clear(); //FIXME: This may not be the best way.
        for (index, t) in self.list.iter_mut().enumerate(){
            if t.should_generate(task,cycle,rng){
                self.index_to_generate[task].push(index);
            }
        }
        // panic if task generates in more than one traffic
        if self.index_to_generate[task].len() > 1{
            panic!("Warning: Multiple traffics are generating messages in the same task.");
        }

        if self.index_to_generate[task].len() > 0{
            let task_state = self.list[self.index_to_generate[task][0]].task_state(task,cycle).expect("Should belong to the traffic");

            self.statistics.track_task_state(task, task_state , cycle, Some(self.index_to_generate[task][0]) );

        }else{

            let mut task_state = Finished;
            let mut t_index = None;
            for (i,traffic) in self.list.iter().enumerate()
            {
                if let Some(state) = traffic.task_state(task,cycle)
                {
                    t_index = Some(i);
                    task_state = state;
                    break;
                }
            }

            self.statistics.track_task_state(task, task_state, cycle, t_index );

        }

        self.index_to_generate[task].len() > 0
    }
    fn task_state(&self, task:usize, cycle:Time) -> Option<TaskTrafficState>
    {
        let mut task_state = None;
        for (_i,traffic) in self.list.iter().enumerate()
        {
            if let Some(state) = traffic.task_state(task,cycle)
            {
                task_state = Some(state);
            }
        }
        if let Some(state) = task_state
        {
            Some(state)
        }else{
            None
        }

    }

    fn number_tasks(&self) -> usize {
        // all traffics have the same number of tasks
        self.tasks
    }
    fn get_statistics(&self) -> Option<TrafficStatistics> {
        Some(self.statistics.clone())
    }
}

impl Sum
{
    pub fn new(mut arg:TrafficBuilderArgument) -> Sum
    {
        let mut list : Option<Vec<_>> =None;
        let mut temporal_step = 0;
        let mut box_size = 1000;
        let mut tasks = None;
        let mut finish_when = None;
        match_object_panic!(arg.cv,"TrafficSum",value,
			"list" => list = Some(value.as_array().expect("bad value for list").iter()
				.map(|v|new_traffic(TrafficBuilderArgument{cv:v,rng:&mut arg.rng,..arg})).collect()),
			"statistics_temporal_step" => temporal_step = value.as_f64().expect("bad value for statistics_temporal_step") as Time,
			"tasks" => tasks = Some(value.as_f64().expect("bad value for tasks") as usize),
			"box_size" => box_size = value.as_f64().expect("bad value for box_size") as usize,
			"finish_when" => finish_when = Some(value.as_array().expect("bad value for finish_when").iter().map(|v|v.as_usize().expect("bad value for finish_when")).collect()),
		);
        let list=list.expect("There were no list");
        assert!( !list.is_empty() , "cannot sum 0 traffics" );
        let size = list[0].number_tasks();
        for traffic in list.iter().skip(1)
        {
            assert_eq!( traffic.number_tasks(), size , "In SumTraffic all sub-traffics must involve the same number of tasks." );
        }
        let finish_when = finish_when.unwrap_or_else(|| (0..list.len()).collect()); //default wait for all
        let tasks = tasks.unwrap();
        let list_statistics = list.iter().map(|_| TrafficStatistics::new(tasks,temporal_step, box_size, None)).collect();
        let statistics = TrafficStatistics::new(tasks,temporal_step, box_size, Some(list_statistics));
        //Debug and print the traffic list
        //println!("TrafficSum list: {:?}", list);
        Sum{
            list,
            index_to_generate: vec![vec![]; tasks ],
            statistics,
            tasks,
            finish_when,
        }
    }
}

/**
The tasks in a ProductTraffic are grouped in blocks of size `block_size`. The traffic each block generates follows the underlying `block_traffic` [Traffic],
but with the group of destination being indicated by the `global_pattern`.
First check whether a transformation at the [Pattern] level is enough; specially see the [crate::pattern::ProductPattern] pattern.

```ignore
ProductTraffic{
	block_size: 10,
	block_traffic: HomogeneousTraffic{...},
	global_pattern: RandomPermutation,
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct ProductTraffic
{
	block_size: usize,
	block_traffic: Box<dyn Traffic>,
	global_pattern: Box<dyn Pattern>,
	global_size: usize,
	//Set of generated messages.
	//generated_messages: BTreeMap<*const Message,Rc<Message>>,
}

impl Traffic for ProductTraffic
{
	fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
	{
		let local=origin % self.block_size;
		let global=origin / self.block_size;
		//let local_dest=self.block_pattern.get_destination(local,topology,rng);
		let global_dest=self.global_pattern.get_destination(global,topology,rng);
		//global_dest*self.block_size+local_dest
		let inner_message=self.block_traffic.generate_message(local,cycle,topology,rng)?;
		let mut payload = Vec::with_capacity(inner_message.payload().len() + 8);
		let destination = global_dest*self.block_size+inner_message.destination;
		payload.extend_from_slice( &(local as u32).to_le_bytes() );
		payload.extend_from_slice( &(destination as u32).to_le_bytes() );
		payload.extend_from_slice(inner_message.payload());
		let outer_message=Rc::new(Message{
			origin,
			destination,
			size:inner_message.size,
			creation_cycle: cycle,
			payload,
		});
		//self.generated_messages.insert(outer_message.as_ref() as *const Message,inner_message);
		Ok(outer_message)
	}
	fn probability_per_cycle(&self,task:usize) -> f32
	{
		let local=task % self.block_size;
		self.block_traffic.probability_per_cycle(local)
	}
	fn try_consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
	{
		//let message_ptr=message.as_ref() as *const Message;
		//let inner_message=match self.generated_messages.remove(&message_ptr)
		//{
		//	None => return false,
		//	Some(m) => m,
		//};
		let mut inner_message = ReferredPayload::from(message);
		inner_message.origin = u32::from_le_bytes( inner_message.payload[0..4].try_into().unwrap() ) as usize;
		inner_message.destination = u32::from_le_bytes( inner_message.payload[4..8].try_into().unwrap() ) as usize;
		inner_message.payload = &inner_message.payload[8..];
		if !self.block_traffic.try_consume(task,&inner_message,cycle,topology,rng)
		{
			panic!("ProductTraffic traffic consumed a message but its child did not.");
		}
		true
	}
	fn is_finished(&self) -> bool
	{
		self.block_traffic.is_finished()
	}
	fn task_state(&self, task:usize, cycle:Time) -> Option<TaskTrafficState>
	{
		let local=task % self.block_size;
		self.block_traffic.task_state(local,cycle)
	}

	fn number_tasks(&self) -> usize {
		self.block_traffic.number_tasks() * self.global_size
	}
}

impl ProductTraffic
{
	pub fn new(mut arg:TrafficBuilderArgument) -> ProductTraffic
	{
		let mut block_size=None;
		let mut block_traffic=None;
		let mut global_pattern=None;
		match_object_panic!(arg.cv,"ProductTraffic",value,
			"block_traffic" => block_traffic=Some(new_traffic(TrafficBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
			"global_pattern" => global_pattern=Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"block_size" => block_size=Some(value.as_f64().expect("bad value for block_size") as usize),
		);
		let block_size=block_size.expect("There were no block_size");
		let block_traffic=block_traffic.expect("There were no block_traffic");
		let mut global_pattern=global_pattern.expect("There were no global_pattern");
		// TODO: should receive a `global_size` argument. When missing, fall back to use topology size.
		// TODO: Also check for divisibility.
		let global_size=arg.topology.num_servers()/block_size;
		global_pattern.initialize(global_size,global_size,arg.topology,arg.rng);
		ProductTraffic{
			block_size,
			block_traffic,
			global_pattern,
			global_size,
			//generated_messages: BTreeMap::new(),
		}
	}
}


///In this traffic each task has a limited amount of data that can send over the amount it has received.
///For example, with `bound=1` after a task sends a message it must wait to receive one.
///And if received `x` messages then it may generate `x+bound` before having to wait.
///All messages have same size, follow the same pattern.
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct BoundedDifference
{
	///Number of tasks applying this traffic.
	tasks: usize,
	///The pattern of the communication.
	pattern: Box<dyn Pattern>,
	///The size of each sent message.
	message_size: usize,
	///The load offered to the network. Proportion of the cycles that should be injecting phits.
	load: f32,
	///The number of messages each task may generate over the amount it has received.
	bound: usize,
	///Set of generated messages.
	generated_messages: BTreeSet<u128>,
	next_id: u128,
	///The number of messages each task is currently allowed to generate until they consume more.
	///It is initialized to `bound`.
	allowance: Vec<usize>,
}

impl Traffic for BoundedDifference
{
	fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
	{
		if origin>=self.tasks
		{
			//panic!("origin {} does not belong to the traffic",origin);
			return Err(TrafficError::OriginOutsideTraffic);
		}
		assert!(self.allowance[origin]>0,"Origin {} has no allowance to send more messages.",origin);
		let destination=self.pattern.get_destination(origin,topology,rng);
		if origin==destination
		{
			return Err(TrafficError::SelfMessage);
		}
		self.allowance[origin]-=1;
		let id = self.next_id;
		self.next_id += 1;
		let message=Rc::new(Message{
			origin,
			destination,
			size:self.message_size,
			creation_cycle: cycle,
			payload: id.to_le_bytes().into(),
		});
		self.generated_messages.insert(id);
		Ok(message)
	}
	fn probability_per_cycle(&self, task:usize) -> f32
	{
		if self.allowance[task]>0
		{
			let r=self.load/self.message_size as f32;
			//println!("load={} r={} size={}",self.load,r,self.message_size);
			if r>1.0
			{
				1.0
			}
			else
			{
				r
			}
		} else { 0f32 }
	}
	fn try_consume(&mut self, task:usize, message: &dyn AsMessage, _cycle:Time, _topology:&dyn Topology, _rng: &mut StdRng) -> bool
	{
		//let message_ptr=message.as_ref() as *const Message;
		self.allowance[task]+=1;
		//self.generated_messages.remove(&message_ptr)
		let id = u128::from_le_bytes(message.payload()[0..16].try_into().expect("bad payload"));
		self.generated_messages.remove(&id)
	}
	fn is_finished(&self) -> bool
	{
		false
	}
	fn task_state(&self, task:usize, _cycle:Time) -> Option<TaskTrafficState>
	{
		if self.allowance[task]>0 {
			Some(Generating)
		} else {
			Some(WaitingData)
		}
	}

	fn number_tasks(&self) -> usize {
		self.tasks
	}
}

impl BoundedDifference
{
	pub fn new(arg:TrafficBuilderArgument) -> BoundedDifference
	{
		let mut tasks=None;
		let mut load=None;
		let mut pattern=None;
		let mut message_size=None;
		let mut bound=None;
		match_object_panic!(arg.cv,"BoundedDifference",value,
			"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"tasks" | "servers" => tasks=Some(value.as_f64().expect("bad value for tasks") as usize),
			"load" => load=Some(value.as_f64().expect("bad value for load") as f32),
			"message_size" => message_size=Some(value.as_f64().expect("bad value for message_size") as usize),
			"bound" => bound=Some(value.as_f64().expect("bad value for bound") as usize),
		);
		let tasks=tasks.expect("There were no tasks");
		let message_size=message_size.expect("There were no message_size");
		let bound=bound.expect("There were no bound");
		let load=load.expect("There were no load");
		let mut pattern=pattern.expect("There were no pattern");
		pattern.initialize(tasks, tasks, arg.topology, arg.rng);
		BoundedDifference{
			tasks,
			pattern,
			message_size,
			load,
			bound,
			generated_messages: BTreeSet::new(),
			allowance: vec![bound;tasks],
			next_id: 0,
		}
	}
}

/**
Traffic which is another shifted by some amount of tasks.
First check whether a transformation at the `Pattern` level is enough.
The task `index+shift` will be seen as just `index` by the inner traffic.
```ignore
ShiftedTraffic{
	traffic: HomogeneousTraffic{...},
	shift: 50,
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Shifted
{
    ///The amount of the shift in tasks.
    shift: usize,
    ///The traffic that is being shifted.
    traffic: Box<dyn Traffic>,
    ///Set of generated messages.
    generated_messages: BTreeMap<*const Message,Rc<Message>>,
}


impl Traffic for Shifted
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        if origin<self.shift
        {
            return Err(TrafficError::OriginOutsideTraffic);
        }
        //let mut message=self.traffic.generate_message(origin-self.shift,rng)?;
        //message.origin=origin;
        //message.destination+=self.shift;
        //Ok(message)
        let inner_message=self.traffic.generate_message(origin-self.shift,cycle,topology,rng)?;
        let outer_message=Rc::new(Message{
            origin,
            destination:inner_message.destination+self.shift,
            size:inner_message.size,
            creation_cycle: cycle,
            cycle_into_network: RefCell::new(None),
        });
        self.generated_messages.insert(outer_message.as_ref() as *const Message,inner_message);
        Ok(outer_message)
    }
    fn probability_per_cycle(&self,task:usize) -> f32
    {
        self.traffic.probability_per_cycle(task-self.shift)
    }
    fn try_consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        let message_ptr=message.as_ref() as *const Message;
        let outer_message=match self.generated_messages.remove(&message_ptr)
        {
            None => return false,
            Some(m) => m,
        };
        if !self.traffic.try_consume(task,outer_message,cycle,topology,rng)
        {
            panic!("Shifted traffic consumed a message but its child did not.");
        }
        true
    }
    fn is_finished(&self) -> bool
    {
        self.traffic.is_finished()
    }
    fn task_state(&self, task:usize, cycle:Time) -> Option<TaskTrafficState>
    {
        self.traffic.task_state(task-self.shift,cycle)
    }

    fn number_tasks(&self) -> usize {
        // TODO: think if this is correct.
        self.traffic.number_tasks()
    }
}

impl Shifted
{
    pub fn new(mut arg:TrafficBuilderArgument) -> Shifted
    {
        let mut shift=None;
        let mut traffic=None;
        match_object_panic!(arg.cv,"ShiftedTraffic",value,
			"traffic" => traffic=Some(new_traffic(TrafficBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
			"shift" => shift=Some(value.as_f64().expect("bad value for shift") as usize),
		);
        let shift=shift.expect("There were no shift");
        let traffic=traffic.expect("There were no traffic");
        Shifted{
            shift,
            traffic,
            generated_messages: BTreeMap::new(),
        }
    }
}
