// Copyright 2022, The Android Open Source Project
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Wrapper around libfdt library. Provides parsing/generating functionality
//! to a bare-metal environment.

#![no_std]

use core::ffi::{c_int, c_void, CStr};
use core::fmt;
use core::mem;
use core::ops::Range;
use core::result;
use core::slice;

/// Error type corresponding to libfdt error codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FdtError {
    /// FDT_ERR_NOTFOUND
    NotFound,
    /// FDT_ERR_EXISTS
    Exists,
    /// FDT_ERR_NOSPACE
    NoSpace,
    /// FDT_ERR_BADOFFSET
    BadOffset,
    /// FDT_ERR_BADPATH
    BadPath,
    /// FDT_ERR_BADPHANDLE
    BadPhandle,
    /// FDT_ERR_BADSTATE
    BadState,
    /// FDT_ERR_TRUNCATED
    Truncated,
    /// FDT_ERR_BADMAGIC
    BadMagic,
    /// FDT_ERR_BADVERSION
    BadVersion,
    /// FDT_ERR_BADSTRUCTURE
    BadStructure,
    /// FDT_ERR_BADLAYOUT
    BadLayout,
    /// FDT_ERR_INTERNAL
    Internal,
    /// FDT_ERR_BADNCELLS
    BadNCells,
    /// FDT_ERR_BADVALUE
    BadValue,
    /// FDT_ERR_BADOVERLAY
    BadOverlay,
    /// FDT_ERR_NOPHANDLES
    NoPhandles,
    /// FDT_ERR_BADFLAGS
    BadFlags,
    /// FDT_ERR_ALIGNMENT
    Alignment,
    /// Unexpected error code
    Unknown(i32),
}

impl fmt::Display for FdtError {
    /// Prints error messages from libfdt.h documentation.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "The requested node or property does not exist"),
            Self::Exists => write!(f, "Attempted to create an existing node or property"),
            Self::NoSpace => write!(f, "Insufficient buffer space to contain the expanded tree"),
            Self::BadOffset => write!(f, "Structure block offset is out-of-bounds or invalid"),
            Self::BadPath => write!(f, "Badly formatted path"),
            Self::BadPhandle => write!(f, "Invalid phandle length or value"),
            Self::BadState => write!(f, "Received incomplete device tree"),
            Self::Truncated => write!(f, "Device tree or sub-block is improperly terminated"),
            Self::BadMagic => write!(f, "Device tree header missing its magic number"),
            Self::BadVersion => write!(f, "Device tree has a version which can't be handled"),
            Self::BadStructure => write!(f, "Device tree has a corrupt structure block"),
            Self::BadLayout => write!(f, "Device tree sub-blocks in unsupported order"),
            Self::Internal => write!(f, "libfdt has failed an internal assertion"),
            Self::BadNCells => write!(f, "Bad format or value of #address-cells or #size-cells"),
            Self::BadValue => write!(f, "Unexpected property value"),
            Self::BadOverlay => write!(f, "Overlay cannot be applied"),
            Self::NoPhandles => write!(f, "Device tree doesn't have any phandle available anymore"),
            Self::BadFlags => write!(f, "Invalid flag or invalid combination of flags"),
            Self::Alignment => write!(f, "Device tree base address is not 8-byte aligned"),
            Self::Unknown(e) => write!(f, "Unknown libfdt error '{e}'"),
        }
    }
}

/// Result type with FdtError enum.
pub type Result<T> = result::Result<T, FdtError>;

fn fdt_err(val: c_int) -> Result<c_int> {
    if val >= 0 {
        Ok(val)
    } else {
        Err(match -val as _ {
            libfdt_bindgen::FDT_ERR_NOTFOUND => FdtError::NotFound,
            libfdt_bindgen::FDT_ERR_EXISTS => FdtError::Exists,
            libfdt_bindgen::FDT_ERR_NOSPACE => FdtError::NoSpace,
            libfdt_bindgen::FDT_ERR_BADOFFSET => FdtError::BadOffset,
            libfdt_bindgen::FDT_ERR_BADPATH => FdtError::BadPath,
            libfdt_bindgen::FDT_ERR_BADPHANDLE => FdtError::BadPhandle,
            libfdt_bindgen::FDT_ERR_BADSTATE => FdtError::BadState,
            libfdt_bindgen::FDT_ERR_TRUNCATED => FdtError::Truncated,
            libfdt_bindgen::FDT_ERR_BADMAGIC => FdtError::BadMagic,
            libfdt_bindgen::FDT_ERR_BADVERSION => FdtError::BadVersion,
            libfdt_bindgen::FDT_ERR_BADSTRUCTURE => FdtError::BadStructure,
            libfdt_bindgen::FDT_ERR_BADLAYOUT => FdtError::BadLayout,
            libfdt_bindgen::FDT_ERR_INTERNAL => FdtError::Internal,
            libfdt_bindgen::FDT_ERR_BADNCELLS => FdtError::BadNCells,
            libfdt_bindgen::FDT_ERR_BADVALUE => FdtError::BadValue,
            libfdt_bindgen::FDT_ERR_BADOVERLAY => FdtError::BadOverlay,
            libfdt_bindgen::FDT_ERR_NOPHANDLES => FdtError::NoPhandles,
            libfdt_bindgen::FDT_ERR_BADFLAGS => FdtError::BadFlags,
            libfdt_bindgen::FDT_ERR_ALIGNMENT => FdtError::Alignment,
            _ => FdtError::Unknown(val),
        })
    }
}

fn fdt_err_expect_zero(val: c_int) -> Result<()> {
    match fdt_err(val)? {
        0 => Ok(()),
        _ => Err(FdtError::Unknown(val)),
    }
}

/// Value of a #address-cells property.
#[derive(Copy, Clone, Debug)]
enum AddrCells {
    Single = 1,
    Double = 2,
}

impl TryFrom<c_int> for AddrCells {
    type Error = FdtError;

    fn try_from(res: c_int) -> Result<Self> {
        match fdt_err(res)? {
            x if x == Self::Single as c_int => Ok(Self::Single),
            x if x == Self::Double as c_int => Ok(Self::Double),
            _ => Err(FdtError::BadNCells),
        }
    }
}

/// Value of a #size-cells property.
#[derive(Copy, Clone, Debug)]
enum SizeCells {
    None = 0,
    Single = 1,
    Double = 2,
}

impl TryFrom<c_int> for SizeCells {
    type Error = FdtError;

    fn try_from(res: c_int) -> Result<Self> {
        match fdt_err(res)? {
            x if x == Self::None as c_int => Ok(Self::None),
            x if x == Self::Single as c_int => Ok(Self::Single),
            x if x == Self::Double as c_int => Ok(Self::Double),
            _ => Err(FdtError::BadNCells),
        }
    }
}

/// Iterator over cells of a DT property.
#[derive(Debug)]
pub struct CellIterator<'a> {
    chunks: slice::ChunksExact<'a, u8>,
}

impl<'a> CellIterator<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        const CHUNK_SIZE: usize = mem::size_of::<<CellIterator as Iterator>::Item>();

        Self { chunks: bytes.chunks_exact(CHUNK_SIZE) }
    }
}

impl<'a> Iterator for CellIterator<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        Some(Self::Item::from_be_bytes(self.chunks.next()?.try_into().ok()?))
    }
}

/// Iterator over a 'reg' property of a DT node.
#[derive(Debug)]
pub struct RegIterator<'a> {
    cells: CellIterator<'a>,
    addr_cells: AddrCells,
    size_cells: SizeCells,
}

/// Represents a contiguous region within the address space defined by the parent bus.
/// Commonly means the offsets and lengths of MMIO blocks, but may have a different meaning on some
/// bus types. Addresses in the address space defined by the root node are CPU real addresses.
#[derive(Copy, Clone, Debug)]
pub struct Reg<T> {
    /// Base address of the region.
    pub addr: T,
    /// Size of the region (optional).
    pub size: Option<T>,
}

impl<'a> RegIterator<'a> {
    fn new(cells: CellIterator<'a>, addr_cells: AddrCells, size_cells: SizeCells) -> Self {
        Self { cells, addr_cells, size_cells }
    }
}

impl<'a> Iterator for RegIterator<'a> {
    type Item = Reg<u64>;

    fn next(&mut self) -> Option<Self::Item> {
        let make_double = |a, b| (u64::from(a) << 32) | u64::from(b);

        let addr = match self.addr_cells {
            AddrCells::Single => self.cells.next()?.into(),
            AddrCells::Double => make_double(self.cells.next()?, self.cells.next()?),
        };
        // If the parent node specifies a value of 0 for #size-cells, 'size' shall be omitted.
        let size = match self.size_cells {
            SizeCells::None => None,
            SizeCells::Single => Some(self.cells.next()?.into()),
            SizeCells::Double => Some(make_double(self.cells.next()?, self.cells.next()?)),
        };

        Some(Self::Item { addr, size })
    }
}

/// Iterator over the address ranges defined by the /memory/ node.
#[derive(Debug)]
pub struct MemRegIterator<'a> {
    reg: RegIterator<'a>,
}

impl<'a> MemRegIterator<'a> {
    fn new(reg: RegIterator<'a>) -> Result<Self> {
        Ok(Self { reg })
    }
}

impl<'a> Iterator for MemRegIterator<'a> {
    type Item = Range<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.reg.next()?;
        let addr = usize::try_from(next.addr).ok()?;
        let size = usize::try_from(next.size?).ok()?;

        Some(addr..addr.checked_add(size)?)
    }
}

/// DT node.
#[derive(Clone, Copy)]
pub struct FdtNode<'a> {
    fdt: &'a Fdt,
    offset: c_int,
}

impl<'a> FdtNode<'a> {
    /// Find parent node.
    pub fn parent(&self) -> Result<Self> {
        // SAFETY - Accesses (read-only) are constrained to the DT totalsize.
        let ret = unsafe { libfdt_bindgen::fdt_parent_offset(self.fdt.as_ptr(), self.offset) };

        Ok(Self { fdt: self.fdt, offset: fdt_err(ret)? })
    }

    /// Retrieve the standard (deprecated) device_type <string> property.
    pub fn device_type(&self) -> Result<&CStr> {
        self.getprop_str(CStr::from_bytes_with_nul(b"device_type\0").unwrap())
    }

    /// Retrieve the standard reg <prop-encoded-array> property.
    pub fn reg(&self) -> Result<RegIterator<'a>> {
        let parent = self.parent()?;

        let addr_cells = parent.address_cells()?;
        let size_cells = parent.size_cells()?;
        let cells = self.getprop_cells(CStr::from_bytes_with_nul(b"reg\0").unwrap())?;

        Ok(RegIterator::new(cells, addr_cells, size_cells))
    }

    /// Retrieve the value of a given <string> property.
    pub fn getprop_str(&self, name: &CStr) -> Result<&CStr> {
        CStr::from_bytes_with_nul(self.getprop(name)?).map_err(|_| FdtError::BadValue)
    }

    /// Retrieve the value of a given property as an array of cells.
    pub fn getprop_cells(&self, name: &CStr) -> Result<CellIterator<'a>> {
        Ok(CellIterator::new(self.getprop(name)?))
    }

    /// Retrieve the value of a given <u32> property.
    pub fn getprop_u32(&self, name: &CStr) -> Result<u32> {
        let prop = self.getprop(name)?.try_into().map_err(|_| FdtError::BadValue)?;
        Ok(u32::from_be_bytes(prop))
    }

    /// Retrieve the value of a given <u64> property.
    pub fn getprop_u64(&self, name: &CStr) -> Result<u64> {
        let prop = self.getprop(name)?.try_into().map_err(|_| FdtError::BadValue)?;
        Ok(u64::from_be_bytes(prop))
    }

    /// Retrieve the value of a given property.
    pub fn getprop(&self, name: &CStr) -> Result<&'a [u8]> {
        let mut len: i32 = 0;
        // SAFETY - Accesses are constrained to the DT totalsize (validated by ctor) and the
        // function respects the passed number of characters.
        let prop = unsafe {
            libfdt_bindgen::fdt_getprop_namelen(
                self.fdt.as_ptr(),
                self.offset,
                name.as_ptr(),
                // *_namelen functions don't include the trailing nul terminator in 'len'.
                name.to_bytes().len().try_into().map_err(|_| FdtError::BadPath)?,
                &mut len as *mut i32,
            )
        } as *const u8;
        if prop.is_null() {
            return fdt_err(len).and(Err(FdtError::Internal));
        }
        let len = usize::try_from(fdt_err(len)?).map_err(|_| FdtError::Internal)?;
        let base =
            (prop as usize).checked_sub(self.fdt.as_ptr() as usize).ok_or(FdtError::Internal)?;

        self.fdt.bytes.get(base..(base + len)).ok_or(FdtError::Internal)
    }

    /// Get reference to the containing device tree.
    pub fn fdt(&self) -> &Fdt {
        self.fdt
    }

    fn next_compatible(self, compatible: &CStr) -> Result<Option<Self>> {
        // SAFETY - Accesses (read-only) are constrained to the DT totalsize.
        let ret = unsafe {
            libfdt_bindgen::fdt_node_offset_by_compatible(
                self.fdt.as_ptr(),
                self.offset,
                compatible.as_ptr(),
            )
        };

        match fdt_err(ret) {
            Ok(offset) => Ok(Some(Self { fdt: self.fdt, offset })),
            Err(FdtError::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn address_cells(&self) -> Result<AddrCells> {
        // SAFETY - Accesses are constrained to the DT totalsize (validated by ctor).
        unsafe { libfdt_bindgen::fdt_address_cells(self.fdt.as_ptr(), self.offset) }
            .try_into()
            .map_err(|_| FdtError::Internal)
    }

    fn size_cells(&self) -> Result<SizeCells> {
        // SAFETY - Accesses are constrained to the DT totalsize (validated by ctor).
        unsafe { libfdt_bindgen::fdt_size_cells(self.fdt.as_ptr(), self.offset) }
            .try_into()
            .map_err(|_| FdtError::Internal)
    }
}

/// Iterator over nodes sharing a same compatible string.
pub struct CompatibleIterator<'a> {
    node: FdtNode<'a>,
    compatible: &'a CStr,
}

impl<'a> CompatibleIterator<'a> {
    fn new(fdt: &'a Fdt, compatible: &'a CStr) -> Result<Self> {
        let node = fdt.root()?;
        Ok(Self { node, compatible })
    }
}

impl<'a> Iterator for CompatibleIterator<'a> {
    type Item = FdtNode<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.node.next_compatible(self.compatible).ok()?;

        if let Some(node) = next {
            self.node = node;
        }

        next
    }
}

/// Wrapper around low-level read-only libfdt functions.
#[repr(transparent)]
pub struct Fdt {
    bytes: [u8],
}

impl Fdt {
    /// Wraps a slice containing a Flattened Device Tree.
    ///
    /// Fails if the FDT does not pass validation.
    pub fn from_slice(fdt: &[u8]) -> Result<&Self> {
        // SAFETY - The FDT will be validated before it is returned.
        let fdt = unsafe { Self::unchecked_from_slice(fdt) };
        fdt.check_full()?;
        Ok(fdt)
    }

    /// Wraps a slice containing a Flattened Device Tree.
    ///
    /// # Safety
    ///
    /// The returned FDT might be invalid, only use on slices containing a valid DT.
    pub unsafe fn unchecked_from_slice(fdt: &[u8]) -> &Self {
        mem::transmute::<&[u8], &Self>(fdt)
    }

    /// Return an iterator of memory banks specified the "/memory" node.
    ///
    /// NOTE: This does not support individual "/memory@XXXX" banks.
    pub fn memory(&self) -> Result<MemRegIterator> {
        let memory = CStr::from_bytes_with_nul(b"/memory\0").unwrap();
        let device_type = CStr::from_bytes_with_nul(b"memory\0").unwrap();

        let node = self.node(memory)?;
        if node.device_type()? != device_type {
            return Err(FdtError::BadValue);
        }

        MemRegIterator::new(node.reg()?)
    }

    /// Retrieve the standard /chosen node.
    pub fn chosen(&self) -> Result<FdtNode> {
        self.node(CStr::from_bytes_with_nul(b"/chosen\0").unwrap())
    }

    /// Get the root node of the tree.
    pub fn root(&self) -> Result<FdtNode> {
        self.node(CStr::from_bytes_with_nul(b"/\0").unwrap())
    }

    /// Find a tree node by its full path.
    pub fn node(&self, path: &CStr) -> Result<FdtNode> {
        let offset = self.path_offset(path)?;
        Ok(FdtNode { fdt: self, offset })
    }

    /// Iterate over nodes with a given compatible string.
    pub fn compatible_nodes<'a>(&'a self, compatible: &'a CStr) -> Result<CompatibleIterator<'a>> {
        CompatibleIterator::new(self, compatible)
    }

    fn path_offset(&self, path: &CStr) -> Result<c_int> {
        let len = path.to_bytes().len().try_into().map_err(|_| FdtError::BadPath)?;
        // SAFETY - Accesses are constrained to the DT totalsize (validated by ctor) and the
        // function respects the passed number of characters.
        let ret = unsafe {
            // *_namelen functions don't include the trailing nul terminator in 'len'.
            libfdt_bindgen::fdt_path_offset_namelen(self.as_ptr(), path.as_ptr(), len)
        };

        fdt_err(ret)
    }

    fn check_full(&self) -> Result<()> {
        let len = self.bytes.len();
        // SAFETY - Only performs read accesses within the limits of the slice. If successful, this
        // call guarantees to other unsafe calls that the header contains a valid totalsize (w.r.t.
        // 'len' i.e. the self.fdt slice) that those C functions can use to perform bounds
        // checking. The library doesn't maintain an internal state (such as pointers) between
        // calls as it expects the client code to keep track of the objects (DT, nodes, ...).
        let ret = unsafe { libfdt_bindgen::fdt_check_full(self.as_ptr(), len) };
        fdt_err_expect_zero(ret)
    }

    fn as_ptr(&self) -> *const c_void {
        self as *const _ as *const c_void
    }
}