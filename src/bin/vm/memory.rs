use crate::constants;

#[derive(Debug)]
pub struct Flags {
    pub zero: bool,
    pub positive: bool,
}

pub struct Memory {
    pub(crate) data: Vec<u8>,
    pub(crate) registers: [u64; 16],
    pub(crate) flags: Flags,
}

impl Memory {
    pub fn new(size: usize, program: Vec<u8>) -> Memory {
        let mut memory = Memory {
            data: vec![0; size],
            registers: [0; 16],
            flags: Flags {
                zero: false,
                positive: false,
            },
        };
        memory.data[..program.len()].copy_from_slice(&program);

        memory.registers[constants::SP] = size as u64;
        memory.registers[constants::BP] = size as u64;

        memory
    }

    pub fn read_u64_le(&self, address: usize, data_size: u8) -> u64 {
        match data_size {
            1 => self.data[address] as u64,
            2 => u16::from_le_bytes([self.data[address], self.data[address + 1]]) as u64,
            4 => u32::from_le_bytes([
                self.data[address],
                self.data[address + 1],
                self.data[address + 2],
                self.data[address + 3],
            ]) as u64,
            8 => u64::from_le_bytes([
                self.data[address],
                self.data[address + 1],
                self.data[address + 2],
                self.data[address + 3],
                self.data[address + 4],
                self.data[address + 5],
                self.data[address + 6],
                self.data[address + 7],
            ]),
            _ => panic!("Invalid data size: {}", data_size),
        }
    }

    pub fn read_i64_le(&self, address: usize, data_size: u8) -> i64 {
        match data_size {
            1 => self.data[address] as i8 as i64,
            2 => i16::from_le_bytes([self.data[address], self.data[address + 1]]) as i64,
            4 => i32::from_le_bytes([
                self.data[address],
                self.data[address + 1],
                self.data[address + 2],
                self.data[address + 3],
            ]) as i64,
            8 => i64::from_le_bytes([
                self.data[address],
                self.data[address + 1],
                self.data[address + 2],
                self.data[address + 3],
                self.data[address + 4],
                self.data[address + 5],
                self.data[address + 6],
                self.data[address + 7],
            ]),
            _ => panic!("Invalid data size: {}", data_size),
        }
    }

    pub fn write_u64_le(&mut self, address: usize, value: u64, data_size: u8) {
        let bytes = match data_size {
            1 => (value as u8).to_le_bytes()[0..1].to_vec(),
            2 => (value as u16).to_le_bytes()[0..2].to_vec(),
            4 => (value as u32).to_le_bytes()[0..4].to_vec(),
            8 => value.to_le_bytes().to_vec(),
            _ => panic!("Invalid data size: {}", data_size),
        };
        self.data[address..address + bytes.len()].copy_from_slice(&bytes);
    }

    pub fn write_i64_le(&mut self, address: usize, value: i64, data_size: u8) {
        let bytes = match data_size {
            1 => (value as i8).to_le_bytes()[0..1].to_vec(),
            2 => (value as i16).to_le_bytes()[0..2].to_vec(),
            4 => (value as i32).to_le_bytes()[0..4].to_vec(),
            8 => value.to_le_bytes().to_vec(),
            _ => panic!("Invalid data size: {}", data_size),
        };
        self.data[address..address + bytes.len()].copy_from_slice(&bytes);
    }

    pub fn read_bytes(&self, address: usize, bytes: usize) -> Vec<u8> {
        self.data[address..address + bytes].to_vec()
    }

    pub fn write_bytes(&mut self, address: usize, bytes: &[u8]) {
        self.data[address..address + bytes.len()].copy_from_slice(bytes);
    }

    pub fn print_registers(&self) {
        println!("Registers: {:?}", self.registers.map(|r| r as i64));
    }

    pub fn print_stack(&self) {
        let sp = self.registers[constants::SP] as usize;
        let bp = self.registers[constants::BP] as usize;
        let stack = self.data[sp..].iter().rev().collect::<Vec<&u8>>();
        println!("Stack: {:?}", stack);
        let stack_frame = &self.data[sp..bp].iter().rev().collect::<Vec<&u8>>();;
        println!("Stack frame: {:?}", stack_frame);
    }
}
