import {expect,it} from 'vitest';import {nextQuestion,resumeQuestion} from '../src/learningFlow';import type {Question} from '../src/api';
const questions=[{id:'where',engine:'sqlite'},{id:'relation',engine:'sqlite'},{id:'mysql',engine:'mysql'},{id:'projection',engine:'sqlite'}] as Question[];
it('continues the last unfinished offline task and advances in course order',()=>{expect(resumeQuestion(questions,{},0,'where')?.id).toBe('where');expect(nextQuestion(questions,{'relation@0':'passed'},0,'relation')?.id).toBe('projection');expect(resumeQuestion(questions,{},0,'mysql')?.id).toBe('relation')});
it('completed offline courses have no artificial restart task',()=>{expect(nextQuestion(questions,{'relation@0':'passed','projection@0':'passed','where@0':'passed'},0)).toBeNull()});
