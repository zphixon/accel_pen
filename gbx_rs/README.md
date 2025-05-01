# gbx_rs

Inspired by https://github.com/thaumictom/gbx-ts

## Notes on GBX file structure

*These might be a little incorrect, please take with a grain of salt*

A .gbx file encodes a gamebox engine node (class) structure. It has the magic bytes
`GBX`, followed by a header, followed by a body of other data (mesh data for
custom blocks, eg) which may or may not be compressed.

### Data types present in GBX files

There are 4 basic data types in GBX files:

- 8-bit integer (u8)
- 16-bit little endian integer (u16)
- 32-bit little endian integer (u32)
- 32-bit floating point number (f32) (little-endian?)

There are also some more complicated data types that appear:

- Character: Just u8
- String:
  - u32 length
  - Sequence of chars (utf-8?)
- Lookback string (see below)
- Int2/Vec2: Two u32s/f32s
- Int3/Vec3: Three u32s/f32s
- Byte3: Three u8s
- Meta: Three lookback strings
  - ID
  - collection
  - author
- File reference
  - Version u8
  - For some versions, a checksum u32
  - Filepath string
  - For some versions, a locator URL
- Node reference
  - Class ID (u32)
  - List of chunks (see below)

#### Lookback strings

Lookback strings are a crude implementation of dictionary compression. They are comprised of

- Lookback version (u32), only if the given lookback string is the first
  occurrence of any lookback string in the file.
- Index (u32)
  - Certain values indicate the empty string, the string "Unassigned", or a
    number of other hardcoded string values.
  - Other values of index indicate that the value of the string (to be used
    again later) follows next, in the normal way (see above).
  - Remaining values (with some bit twiddling) are used to find a string in the
    lookback string table, which should have been inserted earlier.

### Chunks

Data in .gbx files is organized into chunks. Each chunk contains a subset of the
node's attributes (class fields/member variables/instance variables or whatever
you want to call it).

A chunk generally starts with a chunk ID, containing the class ID bit masked
with a chunk type in the lowest three bits. Some chunk types have a version u32
next, which is used to specify what other data is in a chunk.

Data in chunks is simply sequential values of the data types mentioned above.

### The header

Contains metadata about the node class encoded in the file. The header starts
with a fixed section, containing a version number, the class ID, and the number
of chunks in the rest of the header.

Instead of header chunks being prefixed with their chunk IDs, the chunk IDs are
collected together before the chunks in the header.
