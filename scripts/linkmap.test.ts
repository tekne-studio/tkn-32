import { expect, test } from 'bun:test';
import { demangle, regions } from './linkmap';

test('a symbol reads as its path', () => {
	expect(demangle('_RNvCsiDUnzhBDZWi_5rocks4CART')).toBe('rocks::CART');
	expect(demangle('_RNvNtCs6uMctGdEG4A_3dot4look5COVER')).toBe('dot::look::COVER');
	expect(demangle('_ZN3dot5COVER17h0123456789abcdefE')).toBe('dot::COVER');
	expect(demangle('_RNvMNtCsZvADBuc4Ox_9cartridge6screenNtB2_6Screen4rect')).toBe(
		'_RNvMNtCsZvADBuc4Ox_9cartridge6screenNtB2_6Screen4rect'
	);
});

test('a map names the stack and every named static, by address', () => {
	const map = `    Addr      Off     Size Out     In      Symbol
       0        0        0         __stack_pointer
   10000     19ef      ae8 .rodata
   10000     19f7       55         <internal>:(.rodata)
   10a4c     2443        c         /x/dot.o:(.rodata._RNvCs6uMctGdEG4A_3dot5COVER)
   10a4c     2443        c                 _RNvCs6uMctGdEG4A_3dot5COVER
   10ae8     24df       a0 .data
   10ae8     24e7       a0                 _RNvCs6uMctGdEG4A_3dot4CART
   10b88     19ee     4b42 .bss
   10b88        0     4b42                 _RNvCs6uMctGdEG4A_3dot5SHOWN
`;
	expect(regions(map)).toEqual([
		{ at: 0, size: 0x10000, name: 'stack' },
		{ at: 0x10a4c, size: 0xc, name: 'dot::COVER' },
		{ at: 0x10ae8, size: 0xa0, name: 'dot::CART' },
		{ at: 0x10b88, size: 0x4b42, name: 'dot::SHOWN' }
	]);
});
