use crate::AsMessage;
use crate::new_traffic;
use crate::pattern::{new_pattern, PatternBuilderArgument};
use std::collections::{BTreeSet, VecDeque};
use std::convert::TryInto;
use std::rc::Rc;
use quantifiable_derive::Quantifiable;
use rand::prelude::StdRng;
use rand::Rng;
use crate::{match_object_panic, Message, Time};
use crate::pattern::Pattern;
use crate::topology::Topology;
use crate::traffic::{TaskTrafficState, Traffic, TrafficBuilderArgument, TrafficError};
use crate::traffic::TaskTrafficState::{Finished, FinishedGenerating, Generating, UnspecifiedWait};
use crate::ConfigurationValue;

/**
Traffic in which all messages have same size, follow the same pattern, and there is no change with time.

```ignore
HomogeneousTraffic{
	pattern:Uniform,
	tasks:1000,
	load: 0.9,
	message_size: 16,
}
```
**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Homogeneous
{
	///Number of tasks applying this traffic.
	tasks: usize,
	///The pattern of the communication.
	pattern: Box<dyn Pattern>,
	///The size of each sent message.
	message_size: usize,
	///The load offered to the network. Proportion of the cycles that should be injecting phits.
	load: f32,
	///Set of generated messages.
	generated_messages: BTreeSet<u128>,
    ///The id of the next message to generate.
	next_id: u128,
}

impl Traffic for Homogeneous
{
	fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
	{
		if origin>=self.tasks
		{
			//panic!("origin {} does not belong to the traffic",origin);
			return Err(TrafficError::OriginOutsideTraffic);
		}
		let destination=self.pattern.get_destination(origin,topology,rng);
		if origin==destination
		{
			return Err(TrafficError::SelfMessage);
		}
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
		//self.generated_messages.insert(message.as_ref() as *const Message);
		self.generated_messages.insert(id);
		Ok(message)
	}
	fn probability_per_cycle(&self, _task:usize) -> f32
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
	}

    fn consume(&mut self, _task:usize, message: &dyn AsMessage, _cycle:Time, _topology:&dyn Topology, _rng: &mut StdRng) -> bool
    {
        //let message_ptr=message.as_ref() as *const Message;
        //self.generated_messages.remove(&message_ptr)
        let id = u128::from_le_bytes(message.payload()[0..16].try_into().expect("bad payload"));
        self.generated_messages.remove(&id)
    }
    fn is_finished(&self) -> bool
    {
        false
    }
    fn should_generate(&mut self, task: usize, _cycle: Time, rng: &mut StdRng) -> bool {
        let rate= self.probability_per_cycle(task);
        if rate>1.0
        {
            true
        }
        else
        {
            let random= rng.gen_range(0f32..1f32);
            random<rate
        }
    }
	fn task_state(&self, _task:usize, _cycle:Time) -> Option<TaskTrafficState>
	{
		Some(Generating)
	}

	fn number_tasks(&self) -> usize {
		self.tasks
	}
}

impl Homogeneous
{
	pub fn new(arg:TrafficBuilderArgument) -> Homogeneous
	{
		let mut tasks=None;
		let mut load=None;
		let mut pattern=None;
		let mut message_size=None;
		match_object_panic!(arg.cv,"HomogeneousTraffic",value,
			"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"tasks" | "servers" => tasks=Some(value.as_f64().expect("bad value for tasks") as usize),
			"load" => load=Some(value.as_f64().expect("bad value for load") as f32),
			"message_size" => message_size=Some(value.as_f64().expect("bad value for message_size") as usize),
		);
		let tasks=tasks.expect("There were no tasks");
		let message_size=message_size.expect("There were no message_size");
		let load=load.expect("There were no load");
		let mut pattern=pattern.expect("There were no pattern");
		pattern.initialize(tasks, tasks, arg.topology, arg.rng);
		Homogeneous{
			tasks,
			pattern,
			message_size,
			load,
			generated_messages: BTreeSet::new(),
			next_id: 0,
		}
	}
}

/**
Initialize an amount of messages to send from each task.
The traffic will be considered complete when all tasks have generated their messages and all of them have been consumed.

```ignore
Burst{
	pattern:Uniform,
	tasks:1000,
	messages_per_task:200,
	message_size: 16,
    expected_messages_to_consume_per_task (optional): 200, //To have tasks statistics
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Burst
{
    ///Number of tasks applying this traffic.
    tasks: usize,
    ///The pattern of the communication.
    pattern: Box<dyn Pattern>,
    ///The size of each sent message.
    message_size: usize,
    ///The number of messages each task has pending to sent.
    pending_messages: Vec<usize>,
    ///Set of generated messages.
    generated_messages: BTreeSet<u128>,
    ///Expected messages to consume per task
    expected_messages_to_consume: Option<usize>,
    ///Messages per task consumed
    total_consumed_per_task: Vec<usize>,
    ///The id of the next message to generate.
    next_id: u128,
}

impl Traffic for Burst
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        if origin>=self.tasks
        {
            //panic!("origin {} does not belong to the traffic",origin);
            return Err(TrafficError::OriginOutsideTraffic);
        }
        self.pending_messages[origin]-=1;
        let destination=self.pattern.get_destination(origin,topology,rng);
        if origin==destination
        {
            return Err(TrafficError::SelfMessage);
        }
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
        if self.pending_messages[task]>0
        {
            1.0
        }
        else
        {
            0.0
        }
    }

    fn should_generate(&mut self, task:usize, _cycle:Time, _rng: &mut StdRng) -> bool
    {
        self.pending_messages[task]>0
    }

    fn consume(&mut self, task:usize, message: &dyn AsMessage, _cycle:Time, _topology:&dyn Topology, _rng: &mut StdRng) -> bool
    {
        self.total_consumed_per_task[task] += 1;
        let id = u128::from_le_bytes(message.payload()[0..16].try_into().expect("bad payload"));
        self.generated_messages.remove(&id)
    }
    fn is_finished(&self) -> bool
    {
        if !self.generated_messages.is_empty()
        {
            return false;
        }
        for &pm in self.pending_messages.iter()
        {
            if pm>0
            {
                return false;
            }
        }
        true
    }
    fn task_state(&self, task:usize, _cycle:Time) -> Option<TaskTrafficState>
    {
        if self.pending_messages[task]>0 {
            Some(Generating)
        }else{
            if let Some(expected_messages_to_consume) = self.expected_messages_to_consume {
                return if self.total_consumed_per_task[task] < expected_messages_to_consume {
                    Some(FinishedGenerating)
                } else {
                    Some(Finished)
                }
            }
            Some(FinishedGenerating)
        }
    }

    fn number_tasks(&self) -> usize {
        self.tasks
    }
}

impl Burst
{
    pub fn new(arg:TrafficBuilderArgument) -> Burst
    {
        let mut tasks=None;
        let mut messages_per_task=None;
        let mut pattern=None;
        let mut message_size=None;
        let mut expected_messages_to_consume = None;
        match_object_panic!(arg.cv,"Burst",value,
			"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"tasks" | "servers" => tasks=Some(value.as_f64().expect("bad value for tasks") as usize),
			"messages_per_task" | "messages_per_server" => messages_per_task=Some(value.as_f64().expect("bad value for messages_per_task") as usize),
			"message_size" => message_size=Some(value.as_f64().expect("bad value for message_size") as usize),
			"expected_messages_to_consume_per_task" => expected_messages_to_consume=Some(value.as_f64().expect("bad value for expected_messages_to_consume") as usize),
		);
        let tasks=tasks.expect("There were no tasks");
        let message_size=message_size.expect("There were no message_size");
        let messages_per_task=messages_per_task.expect("There were no messages_per_task");
        let mut pattern=pattern.expect("There were no pattern");
        pattern.initialize(tasks, tasks, arg.topology, arg.rng);

        let pending_messages = vec![messages_per_task;tasks];

        Burst{
            tasks,
            pattern,
            message_size,
            pending_messages,
            generated_messages: BTreeSet::new(),
            expected_messages_to_consume,
            total_consumed_per_task: vec![0;tasks],
            next_id: 0,
        }
    }
}

pub struct BuildBurstCVArgs{
    pub tasks: usize,
    pub pattern: ConfigurationValue,
    pub messages_per_task: usize,
    pub message_size: usize,
    pub expected_messages_to_consume_per_task: Option<usize>,
}

pub fn build_burst_cv(args: BuildBurstCVArgs) -> ConfigurationValue {
    let mut cv_list = vec![
        ("tasks".to_string(), ConfigurationValue::Number(args.tasks as f64)),
        ("pattern".to_string(), args.pattern),
        ("messages_per_task".to_string(), ConfigurationValue::Number(args.messages_per_task as f64)),
        ("message_size".to_string(), ConfigurationValue::Number(args.message_size as f64)),
        //("expected_messages_to_consume_per_task".to_string(), ConfigurationValue::Number(args.expected_messages_to_consume_per_task as f64)),
    ];
    if let Some(expected_messages_to_consume_per_task) = args.expected_messages_to_consume_per_task {
        cv_list.push(("expected_messages_to_consume_per_task".to_string(), ConfigurationValue::Number(expected_messages_to_consume_per_task as f64)));
    }
    ConfigurationValue::Object("Burst".to_string(), cv_list)
}


/**
Traffic which allows to generate a specific number of messages in total following a specific traffic.
It finishes when all the messages have been generated and consumed.
Optionally, messages per task could be indicated to restrict all the tasks to generate the same amount of messages.
```ignore
TrafficMessages{
	task:1000,
	traffic: HomogeneousTraffic{...},
	num_messages: 10000,
	messages_per_task: 10, //optional
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct TrafficMessages
{
    ///Number of tasks applying this traffic.
    tasks: usize,
    ///Traffic
    traffic: Box<dyn Traffic>,
    ///The number of messages to send.
    num_messages: usize,
    ///Total sent
    total_sent: usize,
    ///Total consumed
    total_consumed: usize,
    ///Restriction to the number of messages to send per task
    messages_per_task: Option<Vec<usize>>,
    ///The number of messages that a task is expected to consume.
    expected_messages_to_consume: Option<usize>,
    ///Total consumed per task
    total_consumed_per_task: Vec<usize>,
}

impl Traffic for TrafficMessages
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        let message = self.traffic.generate_message(origin,cycle,topology,rng);
        if !message.is_err(){
            self.total_sent += 1;
            if let Some(task_messages) = self.messages_per_task.as_mut() {
                task_messages[origin] -= 1;
                self.total_consumed_per_task[origin] -= 1;
            }
        }
        message
    }
    fn probability_per_cycle(&self, task:usize) -> f32 //should i check the task?
    {
        if self.num_messages > self.total_sent {

            self.traffic.probability_per_cycle(task)

        } else {

            0.0
        }
    }

    fn should_generate(&mut self, task:usize, cycle:Time, rng: &mut StdRng) -> bool
    {
        if let Some(task_messages) = self.messages_per_task.as_ref() {
            self.traffic.should_generate(task, cycle, rng) && task_messages[task] > 0
        }else {
            self.traffic.should_generate(task, cycle, rng) && self.num_messages > self.total_sent
        }
    }

    fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        self.total_consumed += 1;
        self.total_consumed_per_task[task] += 1;
        self.traffic.consume(task, message, cycle, topology, rng)
    }
    fn is_finished(&self) -> bool
    {
        // if self.num_messages <= 0 {
        // 	panic!("TrafficCredit is finished but it should not be.");
        // }
        self.num_messages <= self.total_sent && self.total_sent == self.total_consumed
    }
    fn task_state(&self, task:usize, cycle:Time) -> Option<TaskTrafficState>
    {
        if self.num_messages > self.total_sent && (self.messages_per_task.is_none() || self.messages_per_task.as_ref().unwrap()[task] > 0){
            self.traffic.task_state(task, cycle)
        } else {
            if let Some(expected_messages_to_consume) = self.expected_messages_to_consume {
                return if self.total_consumed_per_task[task] < expected_messages_to_consume {
                    Some(Finished)
                } else {
                    Some(FinishedGenerating)
                }
            }
            Some(FinishedGenerating)
        }
    }

    fn number_tasks(&self) -> usize {
        self.tasks
    }
}

impl TrafficMessages
{
    pub fn new(mut arg:TrafficBuilderArgument) -> TrafficMessages
    {
        let mut tasks=None;
        let mut traffic = None;
        let mut num_messages = None;
        let mut messages_per_task = None;
        let mut expected_messages_to_consume = None;
        match_object_panic!(arg.cv,"Messages",value,
			"traffic" => traffic=Some(new_traffic(TrafficBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
			"tasks" | "servers" => tasks=Some(value.as_usize().expect("bad value for tasks")),
			"num_messages" => num_messages=Some(value.as_usize().expect("bad value for num_messages")),
			"messages_per_task" | "messages_per_server" => messages_per_task=Some(value.as_usize().expect("bad value for messages_per_task")),
			"expected_messages_to_consume_per_task" => expected_messages_to_consume=Some(value.as_usize().expect("bad value for expected_messages_to_consume")),
		);
        let tasks=tasks.expect("There were no tasks");
        let num_messages=num_messages.expect("There were no num_messages");
        let traffic=traffic.expect("There were no traffic");

        let messages_per_task = if messages_per_task.is_some() {
            let mpt = messages_per_task.unwrap();
            if mpt * tasks != num_messages {
                println!("Tasks: {} Messages per task: {} Total messages: {}", tasks, mpt, num_messages);
                panic!("Messages per task and total messages are different.");
            }
            Some(vec![mpt;tasks])
        } else{
            None
        };

        TrafficMessages{
            tasks,
            traffic,
            num_messages,
            total_sent: 0,
            total_consumed: 0,
            messages_per_task,
            expected_messages_to_consume,
            total_consumed_per_task: vec![0; tasks],
        }
    }
}

pub struct BuildMessageCVArgs{
    pub tasks: usize,
    pub traffic: ConfigurationValue,
    pub num_messages: usize,
    pub messages_per_task: Option<usize>,
    pub expected_messages_to_consume_per_task: Option<usize>,
}

pub fn build_message_cv(args: BuildMessageCVArgs) -> ConfigurationValue {
    let mut cv_list = vec![
        ("tasks".to_string(), ConfigurationValue::Number(args.tasks as f64)),
        ("traffic".to_string(), args.traffic),
        ("num_messages".to_string(), ConfigurationValue::Number(args.num_messages as f64)),
    ];
    if let Some(messages_per_task) = args.messages_per_task {
        cv_list.push(("messages_per_task".to_string(), ConfigurationValue::Number(messages_per_task as f64)));
    }
    if let Some(expected_messages_to_consume) = args.expected_messages_to_consume_per_task {
        cv_list.push(("expected_messages_to_consume_per_task".to_string(), ConfigurationValue::Number(expected_messages_to_consume as f64)));
    }
    ConfigurationValue::Object("Messages".to_string(), cv_list)
}


/**

Do nothing until the cycle_to_wake is reached, where the traffic finishes. It is useful to make a task wait for a certain time.
```ignore
Sleep{
    cycle_to_wake: 1000,
    tasks: 1000,
}
```

**/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Sleep
{
    ///Number of tasks applying this traffic.
    cycle_to_wake: Time,
    ///Number of tasks
    tasks: usize,
    /// Is finished
    finished: bool,
}

impl Traffic for Sleep
{
    fn generate_message(&mut self, _origin:usize, _cycle:Time, _topology:&dyn Topology, _rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        return Err(TrafficError::OriginOutsideTraffic);
    }
    fn probability_per_cycle(&self, _task:usize) -> f32
    {
        0.0
    }
    fn consume(&mut self, _task:usize, _message: &dyn AsMessage, _cycle:Time, _topology:&dyn Topology, _rng: &mut StdRng) -> bool
    {
        false
    }
    fn is_finished(&self) -> bool
    {
        self.finished
    }

    fn should_generate(&mut self, _task:usize, cycle:Time, _rng: &mut StdRng) -> bool
    {
        if cycle >= self.cycle_to_wake as u64 {
            self.finished = true;
        }
        false
    }
    fn task_state(&self, _task:usize, _cycle:Time) -> Option<TaskTrafficState>
    {
        Some(TaskTrafficState::UnspecifiedWait)
    }


    fn number_tasks(&self) -> usize {
        self.tasks
    }
}

impl Sleep
{
    pub fn new(arg:TrafficBuilderArgument) -> Sleep
    {
        let mut cycle_to_wake=None;
        let mut tasks=None;

        match_object_panic!(arg.cv,"Sleep",value,
			"cycle_to_wake" => cycle_to_wake=Some(value.as_time().expect("bad value for cycle_to_wake")),
			"tasks" | "servers" => tasks=Some(value.as_f64().expect("bad value for tasks") as usize),
		);
        let cycle_to_wake=cycle_to_wake.expect("There were no cycle_to_wake");
        let tasks=tasks.expect("There were no tasks");
        Sleep {
            cycle_to_wake,
            tasks,
            finished: false,
        }
    }
}


/**
Selects the traffic from a sequence depending on current cycle. This traffics is useful to make sequences of traffics that do no end by themselves.

All the subtraffics in `traffics` must give the same value for `number_tasks`, which is also used for TimeSequenced. At least one such subtraffic must be provided.

```ignore
PeriodicBurst{
	pattern:Uniform,
	period: 2000,
	offset: 0,
	finish: 100000,
	tasks:1000,
	messages_per_task:200,
	message_size: 16,
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct PeriodicBurst
{
    ///times at which the burst will happen
    times_to_generate: VecDeque<Time>,
    ///Number of tasks applying this traffic.
    tasks: usize,
    ///The pattern of the communication.
    pattern: Box<dyn Pattern>,
    ///The size of each sent message.
    message_size: usize,
    ///Messages to send per period
    messages_per_task_per_period: usize,
    ///The number of messages each task has pending to sent.
    pending_messages: Vec<usize>,
    ///Set of generated messages.
    generated_messages: BTreeSet<u128>,
    ///The id of the next message to generate.
    next_id: u128,
}

impl Traffic for PeriodicBurst
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        if origin>=self.tasks
        {
            //panic!("origin {} does not belong to the traffic",origin);
            return Err(TrafficError::OriginOutsideTraffic);
        }
        self.pending_messages[origin]-=1;
        let destination=self.pattern.get_destination(origin,topology,rng);
        if origin==destination
        {
            return Err(TrafficError::SelfMessage);
        }
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
        if self.pending_messages[task]>0
        {
            1.0
        }
        else
        {
            0.0
        }
    }
    fn consume(&mut self, _task:usize, message: &dyn AsMessage, _cycle:Time, _topology:&dyn Topology, _rng: &mut StdRng) -> bool
    {
        let id = u128::from_le_bytes(message.payload()[0..16].try_into().expect("bad payload"));
        self.generated_messages.remove(&id)
    }
    fn is_finished(&self) -> bool
    {
        let times_to_generate = &self.times_to_generate;

        if !self.generated_messages.is_empty() || !times_to_generate.is_empty()
        {
            return false;
        }
        for &pm in self.pending_messages.iter()
        {
            if pm>0
            {
                return false;
            }
        }
        true
    }

    fn should_generate(&mut self, task:usize, cycle:Time, _rng: &mut StdRng) -> bool
    {
        // let mut offset = cycle;
        // let mut traffic_index = 0;
        // while traffic_index<self.traffics.len() && offset >= self.times[traffic_index]
        // {
        // 	offset -= self.times[traffic_index];
        // 	traffic_index += 1;
        // }
        // if traffic_index<self.traffics.len(){
        // 	self.traffics[traffic_index].should_generate(task,cycle,rng)
        // } else {
        // 	false
        // }
        let times = &mut self.times_to_generate;

        if !times.is_empty() && cycle >= times[0] {
            times.pop_front();
            for i in 0..self.pending_messages.len() {
                self.pending_messages[i] += self.messages_per_task_per_period;
            }
        }
        self.pending_messages[task] > 0
    }
    fn task_state(&self, task:usize, _cycle:Time) -> Option<TaskTrafficState>
    {
        if self.pending_messages[task]>0 {
            Some(TaskTrafficState::Generating)
        } else {
            //We do not know whether someone is sending us data.
            if self.is_finished() { Some(TaskTrafficState::Finished) } else { Some(UnspecifiedWait) }
            // Sometimes it could be Finished, but it is not worth computing...
            // TaskTrafficState::UnspecifiedWait
        }
    }


    fn number_tasks(&self) -> usize {
        self.tasks
    }
}

impl PeriodicBurst
{
    pub fn new(arg:TrafficBuilderArgument) -> PeriodicBurst
    {
        let mut pattern=None;
        let mut period=None;
        let mut offset=None;
        let mut finish=None;
        let mut tasks=None;
        let mut messages_per_task_per_period=None;
        let mut message_size=None;
        match_object_panic!(arg.cv,"PeriodicBurst",value,
			"pattern" => pattern=Some(new_pattern(PatternBuilderArgument{cv:value,plugs:arg.plugs})),
			"period" => period = Some(value.as_usize().expect("bad value in period")),
			"offset" => offset = Some(value.as_usize().expect("bad value in offset")),
			"finish" => finish=Some(value.as_usize().expect("bad value for finish")),
			"tasks" => tasks=Some(value.as_f64().expect("bad value for tasks") as usize),
			"messages_per_task_per_period" => messages_per_task_per_period=Some(value.as_f64().expect("bad value for messages_per_task_per_period") as usize),
			"message_size" => message_size=Some(value.as_f64().expect("bad value for message_size") as usize),
		);
        let mut pattern =pattern.expect("There were no pattern");
        let period=period.expect("There were no period");
        let offset=offset.expect("There were no offset");
        let finish=finish.expect("There were no finish");
        let tasks=tasks.expect("There were no tasks");
        let message_size=message_size.expect("There were no message_size");
        let messages_per_task_per_period=messages_per_task_per_period.expect("There were no messages_per_task_per_period");

        let times_to_generate = VecDeque::from((0..((finish-offset)/period +1)).into_iter().map(|i| (i*period + offset) as Time).collect::<Vec<Time>>());
        println!("times_to_generate: {:?}", times_to_generate);
        pattern.initialize(tasks, tasks, arg.topology, arg.rng);
        PeriodicBurst {
            pattern,
            times_to_generate,
            tasks,
            message_size,
            messages_per_task_per_period,
            pending_messages: vec![0;tasks],
            generated_messages: BTreeSet::new(),
            next_id: 0,
        }
    }
}



/**
Only allowed tasks in range will generate messages. The messages can go out of the given range.

```ignore
SubRangeTraffic{
	start: 100,
	end: 200,
	traffic: HomogeneousTraffic{...},
}
```
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct SubRangeTraffic
{
    ///The first element actually in the traffic.
    start: usize,
    ///The next to the last element actually in the traffic.
    end: usize,
    ///The traffic that is being filtered.
    traffic: Box<dyn Traffic>,
    // /Set of generated messages.
    //generated_messages: BTreeMap<*const Message,Rc<Message>>,
}

impl Traffic for SubRangeTraffic
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        if origin<self.start || origin>=self.end
        {
            return Err(TrafficError::OriginOutsideTraffic);
        }
        self.traffic.generate_message(origin,cycle,topology,rng)
    }
    fn probability_per_cycle(&self,task:usize) -> f32
    {
        self.traffic.probability_per_cycle(task)
    }
    fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        self.traffic.consume(task, message, cycle, topology, rng)
    }
    fn is_finished(&self) -> bool
    {
        self.traffic.is_finished()
    }
    fn task_state(&self, task:usize, cycle:Time) -> Option<TaskTrafficState>
    {
        self.traffic.task_state(task,cycle)
    }

    fn number_tasks(&self) -> usize {
        self.traffic.number_tasks()
    }
}

impl SubRangeTraffic
{
    pub fn new(mut arg:TrafficBuilderArgument) -> SubRangeTraffic
    {
        let mut start=None;
        let mut end=None;
        let mut traffic=None;
        match_object_panic!(arg.cv,"SubRangeTraffic",value,
			"traffic" => traffic=Some(new_traffic(TrafficBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
			"start" => start=Some(value.as_f64().expect("bad value for start") as usize),
			"end" => end=Some(value.as_f64().expect("bad value for end") as usize),
		);
        let start=start.expect("There were no start");
        let end=end.expect("There were no end");
        let traffic=traffic.expect("There were no traffic");
        SubRangeTraffic{
            start,
            end,
            traffic,
            //generated_messages: BTreeMap::new(),
        }
    }
}


/**
Has a major traffic `action_traffic` generated normally. When a message from this `action_traffic` is consumed, the `reaction_traffic` is requested for a message. This reaction message will be generated by the task that consumed the action message. The destination of the reaction message is independent of the origin of the action message. The two traffics must involve the same number of tasks.
 **/
#[derive(Quantifiable)]
#[derive(Debug)]
pub struct Reactive
{
    action_traffic: Box<dyn Traffic>,
    reaction_traffic: Box<dyn Traffic>,
    pending_messages: Vec<VecDeque<Rc<Message>>>,
}


impl Traffic for Reactive
{
    fn generate_message(&mut self, origin:usize, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> Result<Rc<Message>,TrafficError>
    {
        if origin<self.pending_messages.len()
        {
            if let Some(message)=self.pending_messages[origin].pop_front()
            {
                return Ok(message);
            }
        }
        return self.action_traffic.generate_message(origin,cycle,topology,rng);
    }
    fn probability_per_cycle(&self, task:usize) -> f32
    {
        if task<self.pending_messages.len() && !self.pending_messages[task].is_empty()
        {
            return 1.0;
        }
        return self.action_traffic.probability_per_cycle(task);
    }
    fn consume(&mut self, task:usize, message: &dyn AsMessage, cycle:Time, topology:&dyn Topology, rng: &mut StdRng) -> bool
    {
        if self.action_traffic.consume(task, message, cycle, topology, rng)
        {
            if self.reaction_traffic.should_generate(message.origin(), cycle, rng)
            {
                match self.reaction_traffic.generate_message(message.origin(), cycle, topology, rng)
                {
                    Ok(response_message) =>
                        {
                            if self.pending_messages.len()<message.origin() +1
                            {
                                self.pending_messages.resize(message.origin() +1, VecDeque::new());
                            }
                            self.pending_messages[message.origin()].push_back(response_message);
                        },
                    //Err(TrafficError::OriginOutsideTraffic) => (),
                    Err(error) => panic!("An error happened when generating response traffic: {:?}",error),
                };
            }
            return true;
        }
        self.reaction_traffic.consume(task, message, cycle, topology, rng)
    }
    fn is_finished(&self) -> bool
    {
        if !self.action_traffic.is_finished() || !self.reaction_traffic.is_finished()
        {
            return false;
        }
        for pm in self.pending_messages.iter()
        {
            if !pm.is_empty()
            {
                return false;
            }
        }
        return true;
    }
    fn task_state(&self, task:usize, cycle:Time) -> Option<TaskTrafficState>
    {
        use TaskTrafficState::*;
        let action_state = self.action_traffic.task_state(task,cycle).expect("TODO! the none case");
        if let Finished = action_state
        {
            return Some(Finished)
        }
        let reaction_state = self.reaction_traffic.task_state(task,cycle).expect("TODO! the none case");
        if let Finished = reaction_state
        {
            return Some(Finished)
        }
        if self.is_finished() { Some(Finished) } else { Some(UnspecifiedWait) }
    }

    fn number_tasks(&self) -> usize {
        // Both traffics have the same number of tasks
        self.action_traffic.number_tasks()
    }
}

impl Reactive
{
    pub fn new(mut arg:TrafficBuilderArgument) -> Reactive
    {
        let mut action_traffic=None;
        let mut reaction_traffic=None;
        match_object_panic!(arg.cv,"Reactive",value,
			"action_traffic" => action_traffic=Some(new_traffic(TrafficBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
			"reaction_traffic" => reaction_traffic=Some(new_traffic(TrafficBuilderArgument{cv:value,rng:&mut arg.rng,..arg})),
		);
        let action_traffic=action_traffic.expect("There were no action_traffic");
        let reaction_traffic=reaction_traffic.expect("There were no reaction_traffic");
        assert_eq!( action_traffic.number_tasks() , reaction_traffic.number_tasks(), "In Reactive both subtraffics should involve the same number of tasks." );
        Reactive{
            action_traffic,
            reaction_traffic,
            pending_messages:vec![],
        }
    }
}