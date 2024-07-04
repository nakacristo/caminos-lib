use crate::packet::ReferredPayload;
use crate::AsMessage;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::convert::TryInto;
use std::rc::Rc;
use quantifiable_derive::Quantifiable;
use rand::prelude::{SliceRandom, StdRng};
use crate::{match_object_panic, Message, Time};
use crate::measures::TrafficStatistics;
use crate::pattern::{new_pattern, Pattern, PatternBuilderArgument};
use crate::topology::Topology;
use crate::traffic::{new_traffic, TaskTrafficState, Traffic, TrafficBuilderArgument, TrafficError};
use crate::traffic::TaskTrafficState::{Generating, WaitingData};
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
        let message = Rc::new(
            Message{
                origin,
                destination: self.from_app_to_machine[app_destination], // get the destination of the message (the machine) from the base map
                size: app_message.size,
                creation_cycle: app_message.creation_cycle,
                payload: app_message.payload().into(),
                id_traffic: app_message.id_traffic,
            }
        );
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

    fn consume(&mut self, task: usize, message: &dyn AsMessage, cycle: Time, topology: &dyn Topology, rng: &mut StdRng) -> bool
    {

        let task_app = self.from_machine_to_app[task].expect("There was no origin for the message");
        let mut app_message = ReferredPayload::from(message);
        app_message.destination = self.from_machine_to_app[app_message.destination].expect("There was no destination for the message");
        app_message.origin = task_app;

        // try to consume the message in the application
        self.application.consume(task_app, &app_message, cycle, topology, rng)
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
	finish_when: [0, 1] // (Optional) finish when the first and second subtraffics are finished. It waits for all by default
    server_task_isolation: false //(Optional) if true, a server can be assigned more than one task. Default is false.
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Sum
{
    ///List of traffic summands
    list: Vec<Box<dyn Traffic>>,
    ///For each task, the index of the traffic that is generating messages.
    index_to_generate: Vec<VecDeque<usize>>,
    ///Statistics for the traffic
    statistics: TrafficStatistics,
    ///Total number of tasks
    tasks:usize,
    ///Indicate the traffics that should be finished to finish the traffic.
    finish_when: Vec<usize>,
    ///Indicate if only one task should be generating messages at a time in the server.
    server_task_isolation: bool,
}

impl Traffic for Sum
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        if self.server_task_isolation && self.index_to_generate[origin].len() > 1{
            panic!("Server task isolation is enabled and there are more than one task generating messages in the server.")
        }

        let index = self.index_to_generate[origin].pop_front().expect("There was no traffic generating a message with this origin.");
        let message = self.list[index].generate_message(origin,cycle,topology,rng);

        if let Ok(ref message) = message{

            let size_msg = message.size;
            self.statistics.track_created_message(cycle, size_msg, Some( index ));

            let mut payload = Vec::with_capacity(message.payload().len() + 4);
            let index_convert = index as u32;
            let i_bytes = bytemuck::bytes_of(&index_convert);
            payload.extend_from_slice(&i_bytes);
            payload.extend_from_slice(message.payload());

            Ok(Rc::from(
                Message {
                    origin: message.origin(),
                    destination: message.destination(),
                    size: message.size(),
                    creation_cycle: message.creation_cycle(),
                    payload,
                    id_traffic: Some(index),
                }
            ))

        } else {
            message
        }

    }

    fn probability_per_cycle(&self,task:usize) -> f32
    {
        self.list.iter().map(|t|t.probability_per_cycle(task)).sum()
    }
    fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        let index=  *bytemuck::try_from_bytes::<u32>(&message.payload()[0..4]).expect("Bad index in message for TrafficSum.") as usize;
        let sub_payload = &message.payload()[4..];

        self.statistics.track_consumed_message(cycle, cycle - message.creation_cycle(), message.size(), Some(index));

        let mut sub_message = ReferredPayload::from(message);
        sub_message.payload = sub_payload;
        self.list[index].consume(task, &sub_message, cycle, topology, rng)
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
        if self.index_to_generate[task].len() > 0{ //TODO: tasks statistics should be checked

            for traffic in self.index_to_generate[task].iter(){
                self.statistics.track_task_state(task, Generating, cycle, Some(*traffic) );
            }

            return true;
        }

        let mut indexes = (0..self.list.len()).collect::<Vec<usize>>();
        indexes.shuffle(rng);

        for index in indexes.iter(){
            if self.list[*index].should_generate(task,cycle,rng){
                self.index_to_generate[task].push_back(*index);
            }
        }

        if self.index_to_generate[task].len() > 0{

            for traffic in self.index_to_generate[task].iter(){
                self.statistics.track_task_state(task, Generating, cycle, Some(*traffic) );
            }

        }else{

            for (i,traffic) in self.list.iter().enumerate()
            {
                if let Some(state) = traffic.task_state(task,cycle)
                {
                    self.statistics.track_task_state(task, state, cycle,  Some(i) );
                }
            }
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
        let mut server_task_isolation = true;
        match_object_panic!(arg.cv,"TrafficSum",value,
			"list" => list = Some(value.as_array().expect("bad value for list").iter()
				.map(|v|new_traffic(TrafficBuilderArgument{cv:v,rng:&mut arg.rng,..arg})).collect()),
			"statistics_temporal_step" => temporal_step = value.as_f64().expect("bad value for statistics_temporal_step") as Time,
			"tasks" => tasks = Some(value.as_f64().expect("bad value for tasks") as usize),
			"box_size" => box_size = value.as_f64().expect("bad value for box_size") as usize,
			"finish_when" => finish_when = Some(value.as_array().expect("bad value for finish_when").iter().map(|v|v.as_usize().expect("bad value for finish_when")).collect()),
            "server_task_isolation" => server_task_isolation = value.as_bool().expect("bad value for server_task_isolation"),
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
            index_to_generate: vec![VecDeque::new(); tasks ],
            statistics,
            tasks,
            finish_when,
            server_task_isolation,
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

        let vec = [local as u32, destination as u32];
        let bytes = bytemuck::bytes_of(&vec);
		// payload.extend_from_slice( &(local as u32).to_le_bytes() );
		// payload.extend_from_slice( &(destination as u32).to_le_bytes() );
        payload.extend_from_slice(bytes);
		payload.extend_from_slice(inner_message.payload());
		let outer_message=Rc::new(Message{
			origin,
			destination,
			size:inner_message.size,
			creation_cycle: cycle,
			payload,
            id_traffic: None,
        });
		//self.generated_messages.insert(outer_message.as_ref() as *const Message,inner_message);
		Ok(outer_message)
	}
	fn probability_per_cycle(&self,task:usize) -> f32
	{
		let local=task % self.block_size;
		self.block_traffic.probability_per_cycle(local)
	}
	fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
	{
		//let message_ptr=message.as_ref() as *const Message;
		//let inner_message=match self.generated_messages.remove(&message_ptr)
		//{
		//	None => return false,
		//	Some(m) => m,
		//};
		let mut inner_message = ReferredPayload::from(message);
        let [origin, destination] = bytemuck::try_cast::<[u8;32],[u32;2]>(message.payload()[0..8].try_into().expect("This should be here!")).expect("MessageTaskSequence: bad payload in consume");

        inner_message.origin = origin as usize;
        inner_message.destination = destination as usize;
		inner_message.payload = &inner_message.payload[8..];
		if !self.block_traffic.consume(task, &inner_message, cycle, topology, rng)
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
    ///The id of the next message to generate.
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
            id_traffic: None,
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
	fn consume(&mut self, task:usize, message: &dyn AsMessage, _cycle:Time, _topology:&dyn Topology, _rng: &mut StdRng) -> bool
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
            payload: inner_message.payload.clone(),
            id_traffic: None,
        });
        //self.generated_messages.insert(outer_message.as_ref() as *const Message,inner_message);
        Ok(outer_message)
    }
    fn probability_per_cycle(&self,task:usize) -> f32
    {
        self.traffic.probability_per_cycle(task-self.shift)
    }
    fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        let mut inner_message = ReferredPayload::from(message);
        inner_message.destination -= self.shift;
        if !self.traffic.consume(task, &inner_message, cycle, topology, rng)
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
