import importlib.util
import sys
import os
import tempfile
import json
import subprocess
from unittest import mock
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent
sys.path.insert(0,str(ROOT))
import corpus
import measure
import access
import experiment
import ratings
import freeze

class CorpusTests(unittest.TestCase):
    def page(self,body="real &amp; text",extra="",namespace="0"):
        return ET.fromstring(f'<page><title>Example</title><ns>{namespace}</ns><id>7</id>{extra}<revision><id>11</id><timestamp>2026-10-01</timestamp><model>wikitext</model><text>{body}</text></revision></page>')

    def test_real_utf8_bytes_identity_and_declared_lf(self):
        d,reason=corpus.article(self.page("自然文 &amp; real"),"")
        self.assertIsNone(reason)
        self.assertEqual(d["data"],"自然文 & real\n".encode())
        self.assertEqual(d["raw_bytes"],len("自然文 & real".encode()))
        self.assertEqual((d["page_id"],d["revision_id"]),(7,11))
        self.assertTrue(d["added_lf"])

    def test_redirect_nonarticle_and_bad_identity(self):
        self.assertIsNone(corpus.article(self.page(extra='<redirect title="Other"/>'),"")[0])
        self.assertIsNone(corpus.article(self.page(namespace="1"),"")[0])
        page=self.page();page.find("id").text="../oracle"
        with self.assertRaises(ValueError):corpus.article(page,"")

    def test_resource_stop_contract(self):
        g=measure.GIB
        self.assertIsNone(measure.stop_reason(1,g,31*g,g,1,0,10))
        for args,reason in [((11,g,31*g,g,1,0,10),"time_budget"),((1,5*g,31*g,g,1,0,10),"sampled_rss_budget"),
                            ((1,g,29*g,g,1,0,10),"free_disk_floor"),((1,g,31*g,31*g,1,0,10),"artifact_budget"),
                            ((1,g,31*g,g,4,0,10),"memory_pressure"),((1,g,31*g,g,2,10,10),"memory_pressure")]:
            self.assertEqual(measure.stop_reason(*args),reason)

    def test_unicode_range_and_no_invented_quote(self):
        data="abc自然文xyz".encode()
        text,start,n,left,right=access.excerpt(data[4:-1],4)
        self.assertEqual(data[start:start+n],text.encode())
        self.assertTrue(left)
        c=experiment.citation("page",data,"自然文")
        self.assertTrue(experiment.check_citation(c,{"page":data})["valid"])
        c['start']=0
        self.assertFalse(experiment.check_citation(c,{"page":data})["valid"])

    def test_unknown_and_not_examined_are_explicit_states(self):
        value={'answers':[{'id':'q1','status':'unknown','answer':'この資料では未確定','citations':[]}], 'investigation':{'q1':'examined'}}
        access.validate(value,['q1'])
        value['investigation']['q1']='guessed'
        with self.assertRaises(ValueError):access.validate(value,['q1'])

    def test_whole_document_subset_bound(self):
        rows=[{'bytes':4},{'bytes':8},{'bytes':2}]
        self.assertEqual(experiment.selected(rows,10),rows[:1])
        with self.assertRaises(ValueError):experiment.selected(rows,3)

    def test_native_symmetric_direct_reads_and_corruption(self):
        binary=os.environ.get('QZT_GIB_BINARY')
        if not binary:self.skipTest('QZT_GIB_BINARY required for native CLI E2E')
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);source=root/'corpus';(source/'documents').mkdir(parents=True);(root/'shards').mkdir()
            texts=[b'alpha evidence entropyX '+b'x'*1500+b' entropy is recorded\n',b'beta evidence remains conditional\n'];rows=[];offset=0
            for i,data in enumerate(texts):
                name=f'p{i}.txt';(source/'documents'/name).write_bytes(data)
                rows.append({'doc_id':name,'title':name,'bytes':len(data),'sha256':corpus.sha(data),'shard':0,'shard_offset':offset,'revision_id':i+1});offset+=len(data)
            (root/'inventory.jsonl').write_text(''.join(json.dumps(d)+'\n' for d in rows));(root/'input.txt').write_bytes(b''.join(texts))
            for args in [[binary,'pack',root/'input.txt','-o',root/'shards/s0000.qzt'],[binary,'sidecar-rebuild',root/'shards/s0000.qzt','--index','token','-o',root/'shards/s0000.qzi']]:
                self.assertEqual(subprocess.run(list(map(str,args)),capture_output=True).returncode,0)
            corpus.save(root/'configuration.json',{'binary':binary,'corpus':str(source),'shards':[{'id':0}]})
            for c in ['B','C']:
                logs=root/c;logs.mkdir();reader=access.Access(root,c,logs)
                self.assertEqual(reader.read('p1.txt',0,100)['text'].encode(),texts[1])
                self.assertEqual(reader.within('p1.txt','conditional')['hits'][0]['doc_id'],'p1.txt')
                hits=reader.search('evidence')['hits'];self.assertEqual({h['doc_id'] for h in hits},{'p0.txt','p1.txt'})
                if c=='B':self.assertGreater(reader.search('entropy')['hits'][0]['start'],1000)
                with self.assertRaises(ValueError):reader.read('../oracle',0,10)
            before=(root/'shards/s0000.qzt').read_bytes();(root/'shards/s0000.qzi').write_bytes(b'broken')
            logs=root/'corrupt';logs.mkdir();reader=access.Access(root,'C',logs)
            with self.assertRaises(ValueError):reader.search('evidence')
            self.assertEqual((root/'shards/s0000.qzi').read_bytes(),b'broken');self.assertEqual((root/'shards/s0000.qzt').read_bytes(),before)

    def test_failed_event_cannot_authorize_unreturned_citation(self):
        import time
        with tempfile.TemporaryDirectory() as t:
            trial=Path(t);(trial/'output/events').mkdir(parents=True)
            corpus.save(trial/'input.json',{'root':t,'condition':'B','questions':[],'seconds':900,'accesses':60,'total_returned_bytes':10000})
            corpus.save(trial/'output/activation.json',{'started_epoch':time.time()})
            corpus.save(trial/'output/events/001.json',{'ok':False,'returned_bytes':0,'fragments':[{'doc_id':'page','start':0,'sha256':'0'*64,'text':'unreturned hidden'}]})
            r=subprocess.run([sys.executable,str(ROOT/'access.py'),str(trial),'cite','page','hidden'],capture_output=True)
            self.assertNotEqual(r.returncode,0)

    def test_missing_answer_failure_is_blinded(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);corpus.save(root/'questions.json',{'questions':[{'id':'q1'}]})
            rows=ratings.responses(root)
            self.assertEqual(len(rows),8)
            self.assertTrue(all('trials' not in r['failure'] and 'B1' not in r['failure'] for r in rows))

    def test_runtime_question_copy_is_in_frozen_scope(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);source=root/'source';(source/'documents').mkdir(parents=True)
            rubric=json.loads((ROOT/'rubric.json').read_text());docs={}
            for q in rubric['questions']:
                for g in q['grounds']:docs.setdefault(g['doc_id'],[]).append(g['quote'])
            rows=[]
            for name,quotes in docs.items():
                data=('\n'.join(quotes)+'\n').encode();(source/'documents'/name).write_bytes(data);rows.append({'doc_id':name,'bytes':len(data)})
            (source/'inventory.jsonl').write_text(''.join(json.dumps(x)+'\n' for x in rows))
            corpus.save(root/'configuration.json',{'corpus':str(source)});corpus.save(root/'calibration.json',{})
            corpus.save(source/'corpus.json',{});corpus.save(source/'acquisition-policy.json',{})
            freeze.main(root);experiment.verify_freeze(root)
            q=json.loads((root/'questions.json').read_text());q['questions'][0]['question']='changed after freeze';(root/'questions.json').write_text(json.dumps(q))
            with self.assertRaises(ValueError):experiment.verify_freeze(root)

    def test_total_deadline_clamps_each_command_and_rejects_overrun(self):
        with mock.patch.object(experiment.measure,'run',return_value={'stop_reason':None,'exit':0}) as run:
            with mock.patch.object(experiment.time,'monotonic',side_effect=[99,101]):
                with self.assertRaises(ValueError):experiment.command(Path('.'),'last',['true'],120,deadline=100)
            self.assertEqual(run.call_args.args[3],1)

    def test_asset_seal_retains_binary_execute_permission(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);source=root/'corpus';(source/'documents').mkdir(parents=True);(root/'material').mkdir();(root/'shards').mkdir()
            binary=root/'material/qzt';binary.write_text('#!/bin/sh\nexit 0\n');binary.chmod(0o755)
            (root/'material/whole.qzt').write_bytes(b'fixture');(root/'inventory.jsonl').write_text('')
            (source/'inventory.jsonl').write_text('');corpus.save(root/'configuration.json',{'corpus':str(source),'binary':str(binary),'binary_sha256':corpus.sha(binary.read_bytes())})
            corpus.save(root/'main-freeze.json',{});corpus.save(root/'questions.json',{'summary':'fixture','questions':[]})
            with mock.patch.object(freeze,'verify_freeze'):freeze.assets(root)
            self.assertFalse(binary.stat().st_mode & 0o222)
            self.assertEqual(subprocess.run([str(binary)],capture_output=True).returncode,0)

    def prepared_fixture(self,root):
        (root/'material').mkdir();binary=root/'material/qzt';binary.write_bytes(b'fixed binary fixture')
        (root/'material/whole.qzt').write_bytes(b'packed fixture')
        (root/'inventory.jsonl').write_text('')
        corpus.save(root/'configuration.json',{'binary':str(binary),'binary_sha256':corpus.sha(binary.read_bytes()),'corpus_bytes':1,'shards':[{'id':0}]})
        corpus.save(root/'main-freeze.json',{})
        return binary

    def test_prepare_rejects_changed_binary_before_native_execution(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);binary=self.prepared_fixture(root);binary.write_bytes(b'different executable')
            with mock.patch.object(experiment,'verify_freeze'),mock.patch.object(experiment,'command',side_effect=AssertionError('changed binary executed')):
                with self.assertRaises(ValueError):experiment.prepare(root)
            self.assertFalse((root/'preparation.json').exists())

    def test_unexpected_whole_index_outcome_cannot_continue_to_shards(self):
        for code,diagnostic in [(0,''),(1,''),(1,'permission denied'),(2,'qzt sidecar-rebuild: a resource limit was exceeded'),(1,'internal error')]:
            with self.subTest(code=code,diagnostic=diagnostic),tempfile.TemporaryDirectory() as t:
                root=Path(t);self.prepared_fixture(root);stderr=root/'stderr';stderr.write_text(diagnostic)
                def fake_command(_root,name,args,*unused,**kw):
                    if name.startswith('s0'):raise AssertionError('unexpected index outcome entered shard preparation')
                    return {'exit':code if name=='whole-index-default' else 0,'stderr':str(stderr)}
                with mock.patch.object(experiment,'verify_freeze'),mock.patch.object(experiment,'command',side_effect=fake_command):
                    with self.assertRaises(ValueError):experiment.prepare(root)
                self.assertFalse((root/'preparation.json').exists())

    def test_expected_resource_refusal_allows_registered_shard_condition(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);self.prepared_fixture(root);(root/'shards').mkdir();stderr=root/'stderr';stderr.write_text('qzt sidecar-rebuild: a resource limit was exceeded\n')
            def fake_command(_root,name,args,*unused,**kw):
                if name=='s0000-pack':(root/'shards/s0000.qzt').write_bytes(b'shard fixture')
                if name=='s0000-index':(root/'shards/s0000.qzi').write_bytes(b'index fixture')
                return {'exit':1 if name=='whole-index-default' else 0,'stderr':str(stderr)}
            with mock.patch.object(experiment,'verify_freeze'),mock.patch.object(experiment,'command',side_effect=fake_command):experiment.prepare(root)
            saved=json.loads((root/'preparation.json').read_text())
            self.assertEqual(saved['whole_index_exit'],1)
            self.assertTrue(saved['shards'])
            self.assertTrue(all(s['exit']==0 for s in saved['shards']))

    def test_binary_change_during_preparation_cannot_save_success(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);binary=self.prepared_fixture(root);cfg=json.loads((root/'configuration.json').read_text());cfg['shards']=[];(root/'configuration.json').write_text(json.dumps(cfg));stderr=root/'stderr';stderr.write_text('qzt sidecar-rebuild: a resource limit was exceeded\n')
            def fake_command(_root,name,args,*unused,**kw):
                if name=='whole-index-default':binary.write_bytes(b'changed during run')
                return {'exit':1 if name=='whole-index-default' else 0,'stderr':str(stderr)}
            with mock.patch.object(experiment,'verify_freeze'),mock.patch.object(experiment,'command',side_effect=fake_command):
                with self.assertRaises(ValueError):experiment.prepare(root)
            self.assertFalse((root/'preparation.json').exists())

    def test_asset_seal_rejects_wrong_declared_binary(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t);binary=self.prepared_fixture(root);source=root/'corpus';(source/'documents').mkdir(parents=True);(source/'inventory.jsonl').write_text('');(root/'inventory.jsonl').write_text('');(root/'shards').mkdir()
            cfg=json.loads((root/'configuration.json').read_text());cfg['corpus']=str(source);(root/'configuration.json').write_text(json.dumps(cfg));binary.write_bytes(b'wrong binary');corpus.save(root/'questions.json',{'summary':'fixture','questions':[]})
            with mock.patch.object(freeze,'verify_freeze'):
                with self.assertRaises(ValueError):freeze.assets(root)
            self.assertFalse((root/'answer-freeze.json').exists())

if __name__=="__main__":unittest.main()
