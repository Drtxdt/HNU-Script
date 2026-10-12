import { defineConfig } from 'vitest/config';
import vue from '@vitejs/plugin-vue';
export default defineConfig({plugins:[vue()],clearScreen:false,server:{port:1420,strictPort:true},build:{target:'es2022',chunkSizeWarningLimit:2500},test:{include:['tests/**/*.test.ts']}});
