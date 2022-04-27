
use std::mem::{size_of};
use crate::quantify::Quantifiable;

///A simple matrix struct. Used for manipulating some matrices of the topology, such as the adjacency matrix and the distance matrix.
#[derive(Debug)]
pub struct Matrix<T>
{
	data: Vec<T>,
	//num_rows: usize,
	num_columns: usize,
}

impl<T> Matrix<T>
{
	///Read a matrix entry.
	pub fn get(&self,row:usize,column:usize) -> &T
	{
		&self.data[row*self.num_columns+column]
	}
	///Read/write a matrix entry.
	pub fn get_mut(&mut self,row:usize,column:usize) -> &mut T
	{
		&mut self.data[row*self.num_columns+column]
	}
	///Get the number of rows
	pub fn get_rows(&self) -> usize
	{
		self.data.len()/self.num_columns
	}
	///Get the number of columns
	pub fn get_columns(&self) -> usize
	{
		self.num_columns
	}
	///Build a matrix with constant values.
	pub fn constant(value:T,num_rows:usize,num_columns:usize) -> Matrix<T> where T:Clone
	{
		Matrix{
			data: vec![value;num_rows*num_columns],
			//num_rows,
			num_columns,
		}
	}
	pub fn map<U,F:FnMut(&T)->U>(&self, f:F) -> Matrix<U>
	{
		Matrix{
			data: self.data.iter().map(f).collect(),
			num_columns: self.num_columns,
		}
	}
}

impl<T:Quantifiable> Quantifiable for Matrix<T>
{
	fn total_memory(&self) -> usize
	{
		size_of::<Matrix<T>>() + self.data.total_memory()
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

